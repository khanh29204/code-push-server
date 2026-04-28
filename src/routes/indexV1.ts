import express from 'express';
import { AppError } from '../core/app-error';
import { Req } from '../core/middleware';
import { clientManager } from '../core/services/client-manager';

// routes for latest code push client
export const indexV1Router = express.Router();

interface UpdateCheckResult {
    packageId: string;
    rollout: number;
    downloadUrl: string;
    description: string;
    isAvailable: boolean;
    isDisabled: boolean;
    appVersion: string;
    label: string;
    packageHash: string;
    packageSize: number;
    shouldRunBinaryVersion: boolean;
    updateAppVersion: string;
    isMandatory: boolean;
}

const formatUpdateInfo = (rs: UpdateCheckResult) => ({
    download_url: rs.downloadUrl,
    description: rs.description,
    is_available: rs.isAvailable,
    is_disabled: rs.isDisabled,
    target_binary_range: rs.appVersion,
    label: rs.label,
    package_hash: rs.packageHash,
    package_size: rs.packageSize,
    should_run_binary_version: rs.shouldRunBinaryVersion,
    update_app_version: rs.updateAppVersion,
    is_mandatory: rs.isMandatory,
});

indexV1Router.get(
    '/update_check',
    async (
        req: Req<
            void,
            void,
            {
                deployment_key: string;
                app_version: string;
                label: string;
                package_hash: string;
                is_companion: unknown;
                client_unique_id: string;
            }
        >,
        res,
        next,
    ): Promise<void> => {
        // <--- Thêm kiểu trả về rõ ràng ở đây
        const { logger, query } = req;
        const {
            deployment_key: deploymentKey,
            app_version: appVersion,
            label,
            package_hash: packageHash,
            client_unique_id: clientUniqueId,
        } = query;

        try {
            logger.info('try update_check', { query: JSON.stringify(query) });

            const rs = (await clientManager.updateCheckFromCache(
                deploymentKey,
                appVersion,
                label,
                packageHash,
                clientUniqueId,
                logger,
            )) as unknown as UpdateCheckResult | null; // <--- Ép kiểu kết quả từ manager

            if (!rs) {
                // Trả về kết quả của res.send() để thỏa mãn linter return value
                res.send({ update_info: { is_available: false } });
                return;
            }

            const rolloutData = await clientManager.chosenMan(
                rs.packageId,
                rs.rollout,
                clientUniqueId,
            );
            if (!rolloutData) {
                rs.isAvailable = false;
            }

            logger.info('update_check success');

            res.send({
                update_info: formatUpdateInfo(rs),
            });
            return;
        } catch (e) {
            if (e instanceof AppError) {
                logger.info('update check failed', { error: e.message });
                res.status(404).send(e.message);
                return;
            }
            next(e);
        }
    },
);

indexV1Router.post(
    '/report_status/download',
    (
        req: Req<
            void,
            {
                client_unique_id: string;
                label: string;
                deployment_key: string;
            },
            void
        >,
        res,
    ) => {
        const { logger, body } = req;
        logger.info('report_status/download', { body: JSON.stringify(body) });
        const { client_unique_id: clientUniqueId, label, deployment_key: deploymentKey } = body;
        clientManager.reportStatusDownload(deploymentKey, label, clientUniqueId).catch((err) => {
            if (err instanceof AppError) {
                logger.info('report_status/download failed', {
                    error: err.message,
                });
            } else {
                logger.error(err);
            }
        });
        res.send('OK');
    },
);

indexV1Router.post(
    '/report_status/deploy',
    (
        req: Req<
            void,
            {
                client_unique_id: string;
                label: string;
                deployment_key: string;
            },
            void
        >,
        res,
    ) => {
        const { logger, body } = req;
        logger.info('report_status/deploy', { body: JSON.stringify(body) });
        const { client_unique_id: clientUniqueId, label, deployment_key: deploymentKey } = body;
        clientManager
            .reportStatusDeploy(deploymentKey, label, clientUniqueId, req.body)
            .catch((err) => {
                if (err instanceof AppError) {
                    logger.info('report_status/deploy failed', {
                        error: err.message,
                    });
                } else {
                    logger.error(err);
                }
            });
        res.send('OK');
    },
);
