import { withOperatorDocs } from '@weebo/operator-docs/next';

// GitHub Pages serves the site under /weebo-kura: the docs workflow passes the
// base path configure-pages reports; `pnpm dev` leaves it empty and serves at /.
export default withOperatorDocs({
  basePath: process.env.DOCS_BASE_PATH ?? '',
});
