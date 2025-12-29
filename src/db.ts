#!/usr/bin/env node
/* eslint-disable no-console */

import 'lodash';
import { config } from './core/config';
import { sequelize } from './core/utils/connections';

// Import tất cả các models để Sequelize nhận diện được schema
import './models/apps';
import './models/collaborators';
import './models/deployments_history';
import './models/deployments_versions';
import './models/deployments';
import './models/log_report_deploy';
import './models/log_report_download';
import './models/packages_diff';
import './models/packages_metrics';
import './models/packages';
import './models/user_tokens';
import { passwordHashSync, randToken } from './core/utils/security';
import { Users } from './models/users';
import { Versions } from './models/versions';

// Hàm khởi tạo DB
const init = async () => {
    console.log(`Initializing database with dialect: ${config.db.dialect}`);
    if (config.db.dialect === 'sqlite') {
        console.log(`Storage file: ${config.db.storage}`);
    }

    try {
        await sequelize.authenticate();
        console.log('Connection has been established successfully.');

        // 1. Tạo bảng (Sync Schema)
        await sequelize.sync({ alter: true });
        console.log('Database synchronized successfully.');

        // 2. Khởi tạo Version DB
        const [, createdVer] = await Versions.findOrCreate({
            where: { type: 1 },
            defaults: { version: '0.5.0' },
        });

        if (createdVer) {
            console.log('Initialized DB Version to 0.5.0');
        }

        // 3. Khởi tạo tài khoản Admin mặc định (Thay thế cho sql/codepush-all.sql)
        // Kiểm tra xem đã có user nào chưa, hoặc kiểm tra user admin cụ thể
        const adminExists = await Users.findOne({ where: { username: 'admin' } });

        if (!adminExists) {
            console.log('Creating default admin account...');
            await Users.create({
                username: 'admin',
                // Mã hóa mật khẩu '123456'
                password: passwordHashSync('123456'),
                email: 'admin@codepush.com',
                identical: randToken(9),
                ack_code: randToken(5),
                created_at: new Date(),
                updated_at: new Date(),
            });
            console.log('-------------------------------------------------------');
            console.log('  Admin account created successfully!');
            console.log('  Username: admin');
            console.log('  Password: 123456');
            console.log('-------------------------------------------------------');
        } else {
            console.log('Admin account already exists. Skipping creation.');
        }

        console.log('SUCCESS: Database is ready.');
        process.exit(0);
    } catch (error) {
        console.error('Unable to connect to the database:', error);
        process.exit(1);
    }
};

init();
