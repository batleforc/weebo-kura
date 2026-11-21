import { defineConfig, globalIgnores } from 'eslint/config';
import nextVitals from 'eslint-config-next/core-web-vitals';

const eslintConfig = defineConfig([
  ...nextVitals,
  globalIgnores([
    '.next/**',
    'out/**',
    'build/**',
    'next-env.d.ts',
    '.source/**',
    // The RFCs' HTML mocks and the OpenAPI document live beside the site.
    'RFC/**',
    'api/**',
  ]),
]);

export default eslintConfig;
