/**
 * CogniCode Backstage backend plugin (CP0 WU4/WU8/WU10/WU11).
 *
 * Responsibilities, exhaustively:
 *   - read `cognicode.baseUrl` from Backstage config
 *   - proxy `GET /probe` to the Rust service over HTTP
 *   - adapt the host's authenticated actor into explicit identity CONTEXT
 *
 * It holds NO canonical CogniCode state and contains NO domain logic: no
 * Findings, no Evidence, no Investigation, no approval or promotion state. It is
 * restartable and replicable at will (WU10), and it never propagates authority.
 */
import express from 'express';
import { coreServices, createBackendPlugin } from '@backstage/backend-plugin-api';
import {
  ACTOR_SOURCE_HEADER,
  ACTOR_SUBJECT_HEADER,
  CogniCodeUnavailableError,
  fetchProbe,
} from '../../../src/cognicode-probe-client';

/** Build the plugin's router. Exported for direct testing. */
export function createCognicodeRouter(deps: {
  baseUrl: string;
  logger: { warn(msg: string): void; error(msg: string): void };
  fetchImpl?: typeof fetch;
}) {
  const router = express.Router();
  const fetchImpl = deps.fetchImpl ?? fetch;

  router.get('/probe', async (req, res) => {
    const subject = req.header(ACTOR_SUBJECT_HEADER);
    try {
      const probe = await fetchProbe({
        baseUrl: deps.baseUrl,
        actor: subject ? { subject, source: 'backstage' } : undefined,
        fetchImpl,
      });
      res.json(probe);
    } catch (error) {
      if (error instanceof CogniCodeUnavailableError) {
        // Unavailable is reported as unavailable — never as stale truth (WU11).
        deps.logger.warn(`CogniCode probe unavailable: ${error.message}`);
        res.status(503).json({ status: 'unavailable', service: 'cognicode' });
        return;
      }
      deps.logger.error(`CogniCode probe failed: ${String(error)}`);
      res.status(502).json({ status: 'error', service: 'cognicode' });
    }
  });

  return router;
}

export const cognicodePlugin = createBackendPlugin({
  pluginId: 'cognicode',
  register(env) {
    env.registerInit({
      deps: {
        logger: coreServices.logger,
        config: coreServices.rootConfig,
        httpRouter: coreServices.httpRouter,
      },
      async init({ logger, config, httpRouter }) {
        const baseUrl = config.getString('cognicode.baseUrl');
        httpRouter.use(createCognicodeRouter({ baseUrl, logger }));
        logger.info(`CogniCode backend plugin ready (service: ${baseUrl})`);
      },
    });
  },
});

export default cognicodePlugin;
