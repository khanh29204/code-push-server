import { Logger } from 'kv-logger';
import _ from 'lodash';
import moment from 'moment';
import validator from 'validator';
import { findCollaboratorsByAppNameAndUid } from '../../models/collaborators';
import { UserTokens } from '../../models/user_tokens';
import { Users } from '../../models/users';
import { AppError } from '../app-error';
import { config } from '../config';
import { redisClient } from '../utils/connections';
import { passwordVerifySync, randToken, md5, passwordHashSync } from '../utils/security';
import { emailManager } from './email-manager';

const LOGIN_LIMIT_PRE = 'LOGIN_LIMIT_PRE_';
const REGISTER_CODE = 'REGISTER_CODE_';
const EXPIRED = 1200;
const EXPIRED_SPEED = 10;

class AccountManager {
    collaboratorCan(uid: number, appName: string, logger: Logger) {
        return this.getCollaborator(uid, appName).then((data) => {
            if (!data) {
                logger.debug(`collaboratorCan App ${appName} not exists.`);
                throw new AppError(`App ${appName} not exists.`);
            }
            return data;
        });
    }

    ownerCan(uid: number, appName: string, logger: Logger) {
        return this.getCollaborator(uid, appName).then((data) => {
            if (!data) {
                logger.debug(`ownerCan App ${appName} not exists.`);
                throw new AppError(`App ${appName} not exists.`);
            }
            if (!_.eq(_.get(data, 'roles'), 'Owner')) {
                logger.debug(`ownerCan Permission Deny, You are not owner!`);
                throw new AppError('Permission Deny, You are not owner!');
            }
            return data;
        });
    }

    findUserByEmail(email: string) {
        return Users.findOne({ where: { email } }).then((data) => {
            if (_.isEmpty(data)) {
                throw new AppError(`${email} does not exist.`);
            } else {
                return data;
            }
        });
    }

    getAllAccessKeyByUid(uid: number) {
        return UserTokens.findAll({
            where: { uid },
            order: [['id', 'DESC']],
        }).then((tokens) => {
            return _.map(tokens, (v) => {
                return {
                    name: '(hidden)',
                    createdTime: moment(v.created_at).valueOf(),
                    createdBy: v.created_by,
                    expires: moment(v.expires_at).valueOf(),
                    friendlyName: v.name,
                    description: v.description,
                };
            });
        });
    }

    isExsitAccessKeyName(uid: number, friendlyName: string) {
        return UserTokens.findOne({
            where: { uid, name: friendlyName },
        });
    }

    createAccessKey(
        uid: number,
        newAccessKey: string,
        ttl: number,
        friendlyName: string,
        createdBy: string,
        description: string,
    ) {
        return UserTokens.create({
            uid,
            name: friendlyName,
            tokens: newAccessKey,
            description,
            created_by: createdBy,
            expires_at: moment()
                .add(ttl / 1000, 'seconds')
                .format('YYYY-MM-DD HH:mm:ss'),
            created_at: moment().format('YYYY-MM-DD HH:mm:ss'),
        });
    }

    login(account: string, password: string) {
        if (_.isEmpty(account)) {
            return Promise.reject(new AppError('Please enter your email address'));
        }
        if (_.isEmpty(password)) {
            return Promise.reject(new AppError('Please enter your password'));
        }
        let where = {};
        if (validator.isEmail(account)) {
            where = { email: account };
        } else {
            where = { username: account };
        }
        const { tryLoginTimes } = config.common;
        return Users.findOne({ where })
            .then((users) => {
                if (_.isEmpty(users)) {
                    throw new AppError('The email or password you entered is incorrect');
                }
                return users;
            })
            .then((users) => {
                if (tryLoginTimes > 0) {
                    const loginKey = `${LOGIN_LIMIT_PRE}${users.id}`;
                    return redisClient.get(loginKey).then((loginErrorTimes) => {
                        if (Number(loginErrorTimes) > tryLoginTimes) {
                            throw new AppError(
                                'The number of times you entered the wrong password exceeds the limit, and the account is locked',
                            );
                        }
                        return users;
                    });
                }
                return users;
            })
            .then((users) => {
                if (!passwordVerifySync(password, users.password)) {
                    if (tryLoginTimes > 0) {
                        const loginKey = `${LOGIN_LIMIT_PRE}${users.id}`;
                        redisClient.exists(loginKey).then((isExists) => {
                            if (!isExists) {
                                const expires = moment().endOf('day').unix() - moment().unix();
                                redisClient.setEx(loginKey, expires, '1');
                                return;
                            }
                            redisClient.incr(loginKey);
                        });
                    }
                    throw new AppError('The email or password you entered is incorrect');
                } else {
                    return users;
                }
            });
    }

    sendRegisterCode(email: string) {
        if (_.isEmpty(email)) {
            return Promise.reject(new AppError('Please enter your email address'));
        }
        return Users.findOne({ where: { email } })
            .then((u) => {
                if (u) {
                    throw new AppError(
                        `"${email}" has already been registered, please use another email address to register`,
                    );
                }
            })
            .then(() => {
                const token = randToken(40);
                return redisClient
                    .setEx(`${REGISTER_CODE}${md5(email)}`, EXPIRED, token)
                    .then(() => {
                        return token;
                    });
            })
            .then((token) => {
                return emailManager.sendRegisterCodeMail(email, token);
            });
    }

    checkRegisterCode(email: string, token: string) {
        return Users.findOne({ where: { email } })
            .then((u) => {
                if (u) {
                    throw new AppError(
                        `"${email}" has already been registered, please use another email address to register`,
                    );
                }
            })
            .then(() => {
                const registerKey = `${REGISTER_CODE}${md5(email)}`;
                return redisClient.get(registerKey).then((storageToken) => {
                    if (_.isEmpty(storageToken)) {
                        throw new AppError(
                            'The verification code has expired, please get it again',
                        );
                    }
                    if (!_.eq(token, storageToken)) {
                        redisClient.ttl(registerKey).then((ttl) => {
                            if (ttl > 0) {
                                redisClient.expire(registerKey, ttl - EXPIRED_SPEED);
                            }
                        });
                        throw new AppError(
                            'The verification code you entered is incorrect, please re-enter it',
                        );
                    }
                    return storageToken;
                });
            });
    }

    register(email: string, password: string) {
        return Users.findOne({ where: { email } })
            .then((u) => {
                if (u) {
                    throw new AppError(
                        `"${email}" has already been registered, please use another email address to register`,
                    );
                }
            })
            .then(() => {
                const identical = randToken(9);
                return Users.create({
                    email,
                    password: passwordHashSync(password),
                    identical,
                });
            });
    }

    changePassword(uid: number, oldPassword: string, newPassword: string) {
        if (!_.isString(newPassword) || newPassword.length < 6) {
            return Promise.reject(
                new AppError('Please enter a new password between 6 and 20 characters long'),
            );
        }
        return Users.findOne({ where: { id: uid } })
            .then((u) => {
                if (!u) {
                    throw new AppError('User information not found');
                }
                return u;
            })
            .then((u) => {
                const isEq = passwordVerifySync(oldPassword, u.get('password'));
                if (!isEq) {
                    throw new AppError(
                        'The old password you entered is incorrect, please re-enter it',
                    );
                }
                u.set('password', passwordHashSync(newPassword));
                u.set('ack_code', randToken(5));
                return u.save();
            });
    }

    private getCollaborator(uid: number, appName: string) {
        return findCollaboratorsByAppNameAndUid(uid, appName);
    }
}

export const accountManager = new AccountManager();
