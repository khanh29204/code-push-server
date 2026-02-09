import { Logger } from 'kv-logger';
import _ from 'lodash';
import { Op } from 'sequelize';
import { Deployments } from '../../models/deployments';
import { DeploymentsVersions } from '../../models/deployments_versions';
import { LogReportDeploy } from '../../models/log_report_deploy';
import { LogReportDownload } from '../../models/log_report_download';
import { Packages } from '../../models/packages';
import { PackagesDiff } from '../../models/packages_diff';
import { PackagesMetrics } from '../../models/packages_metrics';
import { AppError } from '../app-error';
import { config } from '../config';
import { DEPLOYMENT_FAILED, DEPLOYMENT_SUCCEEDED } from '../const';
import { parseVersion, getBlobDownloadUrl } from '../utils/common';
import { redisClient } from '../utils/connections';

const UPDATE_CHECK = 'UPDATE_CHECK';
const CHOSEN_MAN = 'CHOSEN_MAN';
const EXPIRED = 600;

interface UpdateCheckInfo {
    packageId: number;
    downloadURL: string;
    downloadUrl: string;
    description: string;
    isAvailable: boolean;
    isDisabled: boolean;
    isMandatory: boolean;
    appVersion: string;
    targetBinaryRange: string;
    packageHash: string;
    label: string;
    packageSize: number;
    updateAppVersion: boolean;
    shouldRunBinaryVersion: boolean;
    rollout: number;
}

class ClientManager {
    private getUpdateCheckCacheKey(deploymentKey, appVersion, label, packageHash) {
        return [UPDATE_CHECK, deploymentKey, appVersion, label, packageHash].join(':');
    }

    clearUpdateCheckCache(deploymentKey, appVersion, label, packageHash, logger: Logger) {
        logger.info('clear cache Deployments key', {
            key: deploymentKey,
        });
        const redisCacheKey = this.getUpdateCheckCacheKey(
            deploymentKey,
            appVersion,
            label,
            packageHash,
        );
        return redisClient.keys(redisCacheKey).then((data) => {
            if (_.isArray(data)) {
                return Promise.all(
                    data.map((key) => {
                        return redisClient.del(key);
                    }),
                );
            }
            return null;
        });
    }

    updateCheckFromCache(
        deploymentKey: string,
        appVersion: string,
        label: string,
        packageHash: string,
        clientUniqueId: string,
        logger: Logger,
    ) {
        if (!config.common.updateCheckCache) {
            return this.updateCheck(
                deploymentKey,
                appVersion,
                label,
                packageHash,
                clientUniqueId,
                logger,
            );
        }
        const redisCacheKey = this.getUpdateCheckCacheKey(
            deploymentKey,
            appVersion,
            label,
            packageHash,
        );
        return redisClient.get(redisCacheKey).then((data) => {
            if (data) {
                try {
                    logger.debug('updateCheckFromCache read from cache');
                    const obj = JSON.parse(data) as UpdateCheckInfo;
                    return obj;
                } catch (e) {
                    // do nothing
                }
            }
            return this.updateCheck(
                deploymentKey,
                appVersion,
                label,
                packageHash,
                clientUniqueId,
                logger,
            ).then((rs) => {
                try {
                    logger.debug('updateCheckFromCache read from db');
                    const strRs = JSON.stringify(rs);
                    redisClient.setEx(redisCacheKey, EXPIRED, strRs);
                } catch (e) {
                    // do nothing
                }
                return rs;
            });
        });
    }

    private getChosenManCacheKey(packageId, rollout, clientUniqueId) {
        return [CHOSEN_MAN, packageId, rollout, clientUniqueId].join(':');
    }

    private random(rollout) {
        const r = Math.ceil(Math.random() * 10000);
        if (r < rollout * 100) {
            return Promise.resolve(true);
        }
        return Promise.resolve(false);
    }

    chosenMan(packageId, rollout, clientUniqueId: string) {
        if (rollout >= 100) {
            return Promise.resolve(true);
        }
        const rolloutClientUniqueIdCache = _.get(
            config,
            'common.rolloutClientUniqueIdCache',
            false,
        );
        if (rolloutClientUniqueIdCache === false) {
            return this.random(rollout);
        }
        const redisCacheKey = this.getChosenManCacheKey(packageId, rollout, clientUniqueId);
        return redisClient.get(redisCacheKey).then((data) => {
            if (data === '1') {
                return true;
            }
            if (data === '2') {
                return false;
            }
            return this.random(rollout).then((r) => {
                return redisClient
                    .setEx(redisCacheKey, 60 * 60 * 24 * 7, r ? '1' : '2')
                    .then(() => {
                        return r;
                    });
            });
        });
    }

    // eslint-disable-next-line max-lines-per-function
    private async updateCheck(
        deploymentKey: string,
        appVersion: string,
        label: string,
        packageHash: string,
        clientUniqueId: string,
        logger: Logger,
    ): Promise<UpdateCheckInfo | undefined> {
        const startTime = performance.now();

        const rs: UpdateCheckInfo = {
            // ... (Giữ nguyên phần khởi tạo mặc định)
            packageId: 0,
            downloadURL: '',
            downloadUrl: '',
            description: '',
            isAvailable: false,
            isDisabled: true,
            isMandatory: false,
            appVersion,
            targetBinaryRange: '',
            packageHash: '',
            label: '',
            packageSize: 0,
            updateAppVersion: false,
            shouldRunBinaryVersion: false,
            rollout: 100,
        };

        try {
            // 1. Validate & 2. Tìm Deployment & 3. Tìm Version (Giữ nguyên code cũ)
            if (_.isEmpty(deploymentKey) || _.isEmpty(appVersion)) {
                throw new AppError('please input deploymentKey and appVersion');
            }
            const dep = await Deployments.findOne({ where: { deployment_key: deploymentKey } });
            if (!dep) throw new AppError('Not found deployment');

            const version = parseVersion(appVersion);
            const deploymentsVersionsMore = await DeploymentsVersions.findAll({
                where: {
                    deployment_id: dep.id,
                    min_version: { [Op.lte]: version },
                    max_version: { [Op.gt]: version },
                },
            });
            const deploymentsVersions = _.last(_.sortBy(deploymentsVersionsMore, 'created_at'));
            const targetPackageId = _.get(deploymentsVersions, 'current_package_id', 0);

            if (!deploymentsVersions || targetPackageId <= 0) return undefined;

            // 4. Tìm Package đích
            const targetPackage = await Packages.findByPk(targetPackageId);
            if (!targetPackage) return undefined;

            const isSameDeployment =
                targetPackage.deployment_id === deploymentsVersions.deployment_id;
            const isDifferentHash = targetPackage.package_hash !== packageHash;

            if (isSameDeployment && isDifferentHash) {
                // Populate thông tin cơ bản
                rs.packageId = targetPackageId;
                rs.targetBinaryRange = deploymentsVersions.app_version;
                rs.downloadURL = getBlobDownloadUrl(targetPackage.blob_url);
                rs.downloadUrl = rs.downloadURL;
                rs.description = targetPackage.description || '';
                rs.isAvailable = targetPackage.is_disabled !== 1;
                rs.isDisabled = targetPackage.is_disabled === 1;
                rs.isMandatory = targetPackage.is_mandatory === 1;
                rs.appVersion = appVersion;
                rs.packageHash = targetPackage.package_hash;
                rs.label = targetPackage.label;
                rs.packageSize = targetPackage.size;
                rs.rollout = targetPackage.rollout ?? 100;

                // --- [REDIS CACHE LOGIC START] ---
                let finalDescription = rs.description;
                let finalIsMandatory = rs.isMandatory;
                let minId = 0;

                // A. Xác định minId (User đang ở bản nào)
                if (packageHash) {
                    const currentPackage = await Packages.findOne({
                        where: {
                            package_hash: packageHash,
                            deployment_id: targetPackage.deployment_id,
                        },
                    });
                    if (currentPackage) minId = currentPackage.id;
                }

                if (targetPackage.id > minId) {
                    // Tạo Cache Key: Dựa trên ID bản cũ và ID bản mới
                    // Ví dụ: MERGED_INFO:10:20 (Merge từ bản ID 10 lên bản ID 20)
                    const cacheKey = `MERGED_INFO:${packageHash}:${targetPackage.id}`;

                    // B. Thử lấy từ Redis
                    try {
                        const cachedData = await redisClient.get(cacheKey);

                        if (cachedData) {
                            // HIT CACHE: Lấy luôn, không cần query DB nữa
                            logger.debug(`[Cache Hit] ${cacheKey}`);
                            try {
                                const parsed = JSON.parse(cachedData);
                                finalDescription = parsed.description;
                                finalIsMandatory = parsed.isMandatory;
                            } catch (error) {
                                logger.warn(`[Cache] Invalid JSON for ${cacheKey}, deleting...`);
                                redisClient.del(cacheKey);
                            }
                        } else {
                            // MISS CACHE: Query DB và tính toán
                            logger.debug(`[Cache Miss] ${cacheKey} - Querying DB...`);
                            // Kiểm tra mandatory riêng — không giới hạn limit
                            const hasMandatory = await Packages.count({
                                where: {
                                    deployment_id: targetPackage.deployment_id,
                                    id: { [Op.gt]: minId, [Op.lte]: targetPackage.id },
                                    is_disabled: 0,
                                    is_mandatory: 1,
                                },
                            });
                            if (hasMandatory > 0) finalIsMandatory = true;
                            const intermediatePackages = await Packages.findAll({
                                where: {
                                    deployment_id: targetPackage.deployment_id,
                                    id: {
                                        [Op.gt]: minId,
                                        [Op.lte]: targetPackage.id,
                                    },
                                    is_disabled: 0,
                                },
                                order: [['id', 'DESC']],
                                limit: 15,
                            });

                            const messages: string[] = [];
                            intermediatePackages.forEach((pkg) => {
                                if (pkg.description) {
                                    messages.push(`[${pkg.label}]: ${pkg.description}`);
                                }
                            });

                            if (messages.length > 0) {
                                finalDescription = messages.join('\n');
                            }

                            // C. Lưu vào Redis (TTL: 24 giờ = 86400 giây)
                            // Dữ liệu quá khứ không đổi nên có thể cache lâu
                            try {
                                await redisClient.setEx(
                                    cacheKey,
                                    86400,
                                    JSON.stringify({
                                        description: finalDescription,
                                        isMandatory: finalIsMandatory,
                                    }),
                                );
                            } catch (e) {
                                logger.warn(`[Cache] Failed to write ${cacheKey}`, { error: e });
                            }
                        }
                    } catch (cacheError) {
                        logger.warn('[Cache] Redis error, falling back to DB', {
                            error: cacheError,
                        });
                    }
                }

                rs.description = finalDescription;
                rs.isMandatory = finalIsMandatory;

                // 5. Kiểm tra Diff Update
                if (packageHash) {
                    const diffPackage = await PackagesDiff.findOne({
                        where: {
                            package_id: targetPackage.id,
                            diff_against_package_hash: packageHash,
                        },
                    });
                    if (diffPackage) {
                        const diffUrl = getBlobDownloadUrl(diffPackage.diff_blob_url);
                        rs.downloadURL = diffUrl;
                        rs.downloadUrl = diffUrl;
                        rs.packageSize = diffPackage.diff_size;
                    }
                }
            }
            return rs;
        } finally {
            const endTime = performance.now();
            const duration = (endTime - startTime).toFixed(2);
            if (Number(duration) > 200) {
                logger.warn(`[Slow Query] updateCheck took ${duration}ms`, {
                    deploymentKey,
                    appVersion,
                });
            } else {
                logger.debug(`[Perf] updateCheck took ${duration}ms`);
            }
        }
    }

    private getPackagesInfo(deploymentKey, label) {
        if (_.isEmpty(deploymentKey) || _.isEmpty(label)) {
            return Promise.reject(new AppError('please input deploymentKey and label'));
        }
        return Deployments.findOne({ where: { deployment_key: deploymentKey } })
            .then((dep) => {
                if (_.isEmpty(dep)) {
                    throw new AppError('does not found deployment');
                }
                return Packages.findOne({ where: { deployment_id: dep.id, label } });
            })
            .then((packages) => {
                if (_.isEmpty(packages)) {
                    throw new AppError('does not found packages');
                }
                return packages;
            });
    }

    reportStatusDownload(deploymentKey, label, clientUniqueId) {
        return this.getPackagesInfo(deploymentKey, label).then((packages) => {
            return Promise.all([
                PackagesMetrics.findOne({ where: { package_id: packages.id } }).then((metrics) => {
                    if (metrics) {
                        return metrics.increment('downloaded');
                    }
                    return undefined;
                }),
                LogReportDownload.create({
                    package_id: packages.id,
                    client_unique_id: clientUniqueId,
                }),
            ]);
        });
    }

    reportStatusDeploy(deploymentKey: string, label, clientUniqueId: string, others) {
        return this.getPackagesInfo(deploymentKey, label).then((packages) => {
            const statusText = _.get(others, 'status');
            let status = 0;
            if (_.eq(statusText, 'DeploymentSucceeded')) {
                status = DEPLOYMENT_SUCCEEDED;
            } else if (_.eq(statusText, 'DeploymentFailed')) {
                status = DEPLOYMENT_FAILED;
            }
            const packageId = packages.id;
            const previousDeploymentKey = _.get(others, 'previousDeploymentKey');
            const previousLabel = _.get(others, 'previousLabelOrAppVersion');
            if (status > 0) {
                return Promise.all([
                    LogReportDeploy.create({
                        package_id: packageId,
                        client_unique_id: clientUniqueId,
                        previous_label: previousLabel,
                        previous_deployment_key: previousDeploymentKey,
                        status,
                    }),
                    PackagesMetrics.findOne({ where: { package_id: packageId } }).then(
                        (metrics) => {
                            if (_.isEmpty(metrics)) {
                                return undefined;
                            }
                            if (_.eq(status, DEPLOYMENT_SUCCEEDED)) {
                                return metrics.increment(['installed', 'active'], { by: 1 });
                            }
                            return metrics.increment(['installed', 'failed'], { by: 1 });
                        },
                    ),
                ]).then(() => {
                    if (previousDeploymentKey && previousLabel) {
                        return Deployments.findOne({
                            where: { deployment_key: previousDeploymentKey },
                        })
                            .then((dep) => {
                                if (_.isEmpty(dep)) {
                                    return undefined;
                                }
                                return Packages.findOne({
                                    where: { deployment_id: dep.id, label: previousLabel },
                                }).then((p) => {
                                    if (_.isEmpty(p)) {
                                        return undefined;
                                    }
                                    return PackagesMetrics.findOne({
                                        where: { package_id: p.id },
                                    });
                                });
                            })
                            .then((metrics) => {
                                if (metrics) {
                                    return metrics.decrement('active');
                                }
                                return undefined;
                            });
                    }
                    return undefined;
                });
            }
            return undefined;
        });
    }
}

export const clientManager = new ClientManager();
