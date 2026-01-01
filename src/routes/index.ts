import fs from 'fs';
import path from 'path';
import express from 'express';
import { AppError } from '../core/app-error';
import { config } from '../core/config';
import { i18n } from '../core/i18n';
import { checkToken, Req } from '../core/middleware';
import { clientManager } from '../core/services/client-manager';
import { Packages } from '../models/packages';

export const indexRouter = express.Router();

indexRouter.get('/', (req, res) => {
    res.render('index', { title: 'CodePushServer' });
});

indexRouter.get('/tokens', (req, res) => {
    // eslint-disable-next-line no-underscore-dangle
    res.render('tokens', { title: `${i18n.__('Obtain')} token` });
});

indexRouter.get(
    '/updateCheck',
    (
        req: Req<
            void,
            void,
            {
                deploymentKey: string;
                appVersion: string;
                label: string;
                packageHash: string;
                clientUniqueId: string;
            }
        >,
        res,
        next,
    ) => {
        const { logger, query } = req;
        logger.info('updateCheck', {
            query: JSON.stringify(query),
        });
        const { deploymentKey, appVersion, label, packageHash, clientUniqueId } = query;
        clientManager
            .updateCheckFromCache(
                deploymentKey,
                appVersion,
                label,
                packageHash,
                clientUniqueId,
                logger,
            )
            .then((rs) => {
                return clientManager
                    .chosenMan(rs.packageId, rs.rollout, clientUniqueId)
                    .then((data) => {
                        if (!data) {
                            rs.isAvailable = false;
                            return rs;
                        }
                        return rs;
                    });
            })
            .then((rs) => {
                logger.info('updateCheck success');

                delete rs.packageId;
                delete rs.rollout;
                res.send({ updateInfo: rs });
            })
            .catch((e) => {
                if (e instanceof AppError) {
                    logger.info('updateCheck failed', {
                        error: e.message,
                    });
                    res.status(404).send(e.message);
                } else {
                    next(e);
                }
            });
    },
);

indexRouter.post(
    '/reportStatus/download',
    (
        req: Req<
            void,
            {
                clientUniqueId: string;
                label: string;
                deploymentKey: string;
            },
            void
        >,
        res,
    ) => {
        const { logger, body } = req;
        logger.info('reportStatus/download', {
            body: JSON.stringify(body),
        });
        const { clientUniqueId, label, deploymentKey } = body;
        clientManager.reportStatusDownload(deploymentKey, label, clientUniqueId).catch((err) => {
            if (err instanceof AppError) {
                logger.info('reportStatus/deploy failed', {
                    error: err.message,
                });
            } else {
                logger.error(err);
            }
        });
        res.send('OK');
    },
);

indexRouter.post(
    '/reportStatus/deploy',
    (
        req: Req<
            void,
            {
                clientUniqueId: string;
                label: string;
                deploymentKey: string;
            },
            void
        >,
        res,
    ) => {
        const { logger, body } = req;
        logger.info('reportStatus/deploy', {
            body: JSON.stringify(body),
        });
        const { clientUniqueId, label, deploymentKey } = body;
        clientManager
            .reportStatusDeploy(deploymentKey, label, clientUniqueId, req.body)
            .catch((err) => {
                if (err instanceof AppError) {
                    logger.info('reportStatus/deploy failed', {
                        error: err.message,
                    });
                } else {
                    logger.error(err);
                }
            });
        res.send('OK');
    },
);

indexRouter.get('/authenticated', checkToken, (req, res) => {
    return res.send({ authenticated: true });
});

const scanFilesRecursively = (
    dir: string,
    fileList: { name: string; size: number; path: string; modified: number }[] = [],
) => {
    try {
        const files = fs.readdirSync(dir);
        files.forEach((file) => {
            // Bỏ qua file ẩn hệ thống .DS_Store, .gitkeep...
            if (file.startsWith('.')) return;

            const fullPath = path.join(dir, file);
            const stat = fs.statSync(fullPath);

            if (stat.isDirectory()) {
                // Nếu là thư mục, đệ quy quét tiếp bên trong
                scanFilesRecursively(fullPath, fileList);
            } else {
                // Nếu là file, thêm vào danh sách
                fileList.push({
                    name: path.basename(file), // Lấy tên file làm Hash (CodePush lưu hash là tên file)
                    size: stat.size,
                    path: fullPath,
                    modified: stat.mtimeMs,
                });
            }
        });
    } catch (e) {
        // Bỏ qua lỗi nếu không đọc được thư mục con nào đó
        // eslint-disable-next-line no-console
        console.error(`Error scanning dir ${dir}:`, e);
    }
    return fileList;
};

// eslint-disable-next-line max-lines-per-function
indexRouter.get('/storage/audit', checkToken, async (req, res, next) => {
    const { logger } = req as Req;

    if (config.common.storageType !== 'local') {
        next(new AppError(`Audit API not supported for storageType: ${config.common.storageType}`));
    }

    const { storageDir } = config.local;

    try {
        logger.info(`[Audit] Scanning directory (Recursive FS): ${storageDir}`);

        if (!fs.existsSync(storageDir)) {
            throw new AppError(`Storage directory does not exist: ${storageDir}`);
        }

        // 1. Quét file từ ổ cứng (Dùng hàm thủ công mới viết)
        const scannedFiles = scanFilesRecursively(storageDir);

        logger.info(`[Audit] Found ${scannedFiles.length} files on disk.`);

        // 2. Lấy dữ liệu từ DB
        const dbPackages = await Packages.findAll({
            attributes: ['id', 'label', 'package_hash', 'blob_url', 'manifest_blob_url'],
            raw: true,
        });

        logger.info(`[Audit] Found ${dbPackages.length} packages in DB.`);

        // 3. Xử lý dữ liệu Disk để so sánh
        const diskFileMap = new Map();
        let totalSizeBytes = 0;

        scannedFiles.forEach((file) => {
            totalSizeBytes += file.size;
            // Key là tên file (hash) để tra cứu cho nhanh
            diskFileMap.set(file.name, {
                hash: file.name,
                size: file.size,
                fullPath: file.path,
                modifiedTime: file.modified,
            });
        });

        const report = {
            summary: {
                totalFilesOnDisk: diskFileMap.size,
                totalSizeOnDisk: `${(totalSizeBytes / 1024 / 1024).toFixed(2)} MB`,
                totalPackagesInDb: dbPackages.length,
                orphanedFilesCount: 0,
                missingFilesCount: 0,
                scanPath: storageDir,
            },
            orphanedFiles: [] as unknown[],
            missingFiles: [] as unknown[],
            validFiles: 0,
        };

        const validHashes = new Set<string>();

        // 4. So khớp DB -> Disk
        dbPackages.forEach((pkg) => {
            const checkFile = (hash: string, type: 'blob' | 'manifest') => {
                if (!hash) return;

                validHashes.add(hash);

                if (diskFileMap.has(hash)) {
                    report.validFiles += 1;
                } else {
                    report.missingFiles.push({
                        packageId: pkg.id,
                        label: pkg.label,
                        type,
                        hash,
                        // Thêm hint đường dẫn file lẽ ra phải nằm ở đâu để dễ debug
                        expectedPath: path.join(
                            storageDir,
                            hash.substring(0, 2).toLowerCase(),
                            hash,
                        ),
                    });
                }
            };

            checkFile(pkg.blob_url, 'blob');
            checkFile(pkg.manifest_blob_url, 'manifest');
        });

        report.summary.missingFilesCount = report.missingFiles.length;

        // 5. So khớp Disk -> DB (Tìm file rác)
        diskFileMap.forEach((info, hash) => {
            if (!validHashes.has(hash)) {
                report.orphanedFiles.push({
                    hash,
                    size: info.size,
                    path: info.fullPath,
                    // Convert timestamp sang string dễ đọc
                    modified: new Date(info.modifiedTime).toISOString(),
                });
            }
        });

        report.summary.orphanedFilesCount = report.orphanedFiles.length;

        res.send(report);
    } catch (e) {
        logger.error('[Audit] Error', e);
        next(e);
    }
});
