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

// eslint-disable-next-line max-lines-per-function
indexRouter.get('/storage/audit', checkToken, async (req, res, next) => {
    const { logger } = req as Req;

    if (config.common.storageType !== 'local') {
        next(new AppError('This API is only available when storageType is "local".'));
        return;
    }

    const { storageDir } = config.local;

    try {
        logger.info(`[Audit] Starting storage audit (Bun Native) in: ${storageDir}`);

        // BUN NATIVE: Kiểm tra thư mục tồn tại
        // Bun không có hàm existsSync trực tiếp cho Dir, dùng fs hoặc check stat
        // Nhưng để thuần Bun, ta dùng Glob quét luôn, nếu lỗi nghĩa là ko có.

        // 2. Scan Disk bằng Bun.Glob (Nhanh hơn recursive-readdir nhiều)
        const glob = new Bun.Glob('**/*');

        // Chạy song song: Quét file & Query DB
        const [scannedFiles, dbPackages] = await Promise.all([
            // Scan disk trả về Async Iterator
            (async () => {
                const files = await Promise.all(
                    // eslint-disable-next-line @typescript-eslint/ban-ts-comment
                    // @ts-ignore
                    Array.from(glob.scan({ cwd: storageDir, onlyFiles: true }))
                        .filter((fileName: string) => !fileName.startsWith('.')) // Bỏ qua file ẩn
                        .map(async (fileName: string) => {
                            const fullPath = path.join(storageDir, fileName);
                            const file = Bun.file(fullPath);
                            return {
                                name: path.basename(fileName), // Hash
                                size: file.size,
                                path: fullPath,
                                modified: file.lastModified,
                            };
                        }),
                );
                return files;
            })(),
            Packages.findAll({
                attributes: ['id', 'label', 'package_hash', 'blob_url', 'manifest_blob_url'],
                raw: true,
            }),
        ]);

        // 3. Process Logic (Giống phiên bản trước nhưng dùng dữ liệu từ Bun)
        const diskFileMap = new Map();
        let totalSizeBytes = 0;

        scannedFiles.forEach((file) => {
            totalSizeBytes += file.size;
            diskFileMap.set(file.name, {
                hash: file.name,
                size: file.size,
                fullPath: file.path,
                modifiedTime: file.modified,
            });
        });

        // 4. Compare (Logic so sánh giữ nguyên)
        const report = {
            summary: {
                totalFilesOnDisk: diskFileMap.size,
                totalSizeOnDisk: `${(totalSizeBytes / 1024 / 1024).toFixed(2)} MB`,
                totalPackagesInDb: dbPackages.length,
                orphanedFilesCount: 0,
                missingFilesCount: 0,
            },
            orphanedFiles: [],
            missingFiles: [],
        };

        const validHashes = new Set<string>();

        dbPackages.forEach((pkg) => {
            const check = (hash: string, type: string) => {
                if (!hash) return;
                validHashes.add(hash);
                if (!diskFileMap.has(hash)) {
                    report.missingFiles.push({
                        packageId: pkg.id,
                        label: pkg.label,
                        type,
                        hash,
                    });
                }
            };
            check(pkg.blob_url, 'blob');
            check(pkg.manifest_blob_url, 'manifest');
        });

        report.summary.missingFilesCount = report.missingFiles.length;

        diskFileMap.forEach((info, hash) => {
            if (!validHashes.has(hash)) {
                report.orphanedFiles.push(info);
            }
        });

        report.summary.orphanedFilesCount = report.orphanedFiles.length;

        res.send(report);
    } catch (e) {
        logger.error('[Audit] Error', e);
        next(e);
    }
});
