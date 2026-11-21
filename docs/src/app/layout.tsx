import type { Metadata } from 'next';
import './global.css';
import { JetBrains_Mono, Silkscreen } from 'next/font/google';
import { DocsRootProvider, siteMetadata } from '@weebo/operator-docs/site';
import { site } from '@/lib/shared';

// Absolute base of the OG images and canonical URLs, Pages sub-path included;
// the docs workflow passes configure-pages' origin and base path.
export const metadata: Metadata = siteMetadata(
  site,
  new URL(`${process.env.DOCS_SITE_URL ?? 'https://batleforc.github.io'}${process.env.DOCS_BASE_PATH ?? '/weebo-kura'}`),
);

// The weebo sites' pairing: a square-pixel display face, a mono text face.
const silkscreen = Silkscreen({
  subsets: ['latin'],
  weight: ['400', '700'],
  variable: '--font-silkscreen',
  display: 'swap',
});

const jetbrainsMono = JetBrains_Mono({
  subsets: ['latin'],
  weight: ['400', '500', '600', '700', '800'],
  variable: '--font-jetbrains-mono',
  display: 'swap',
});

export default function Layout({ children }: LayoutProps<'/'>) {
  return (
    <html
      lang="en"
      className={`${silkscreen.variable} ${jetbrainsMono.variable} font-sans`}
      suppressHydrationWarning
    >
      <body className="flex flex-col min-h-screen">
        <DocsRootProvider>{children}</DocsRootProvider>
      </body>
    </html>
  );
}
