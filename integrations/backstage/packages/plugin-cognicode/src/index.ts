/**
 * CogniCode Backstage frontend plugin (CP0 WU4/WU5/WU6).
 *
 * Uses the MODERN frontend system (`createFrontendPlugin` + `PageBlueprint`),
 * not the legacy `createPlugin`/`createRoutableExtension` APIs.
 *
 * The page is a shell: it shows what the Rust service reports and offers an
 * `Open Explorer` handoff carrying a ContextCapsule. It embeds NO Explorer UI
 * (no iframe) and holds no CogniCode state.
 *
 * STATUS: authored against the documented Backstage frontend-system API. In the
 * CP0 spike environment the package is ESM-only with React peer conditions that
 * Node could not resolve, so this file was NOT built or typechecked here. See
 * openspec/changes/cp0-backstage-fit-spike/verification-report.md.
 */
import React from 'react';
import {
  PageBlueprint,
  createFrontendPlugin,
} from '@backstage/frontend-plugin-api';
import { buildContextCapsule, explorerDeepLink } from '../../../src/context-capsule';

export const cognicodePage = PageBlueprint.make({
  params: {
    path: '/cognicode',
    title: 'CogniCode',
    loader: async () => {
      const { CogniCodePage } = await import('./CogniCodePage');
      return <CogniCodePage />;
    },
  },
});

export const cognicodePlugin = createFrontendPlugin({
  pluginId: 'cognicode',
  extensions: [cognicodePage],
});

export { buildContextCapsule, explorerDeepLink };
export default cognicodePlugin;
