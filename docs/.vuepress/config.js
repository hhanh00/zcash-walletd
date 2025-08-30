import { viteBundler } from "@vuepress/bundler-vite";
import { defaultTheme } from "@vuepress/theme-default";
import { defineUserConfig } from "vuepress";
import { markdownImagePlugin } from "@vuepress/plugin-markdown-image";
import { markdownMathPlugin } from "@vuepress/plugin-markdown-math";
import { markdownHintPlugin } from "@vuepress/plugin-markdown-hint";
import { markdownExtPlugin } from "@vuepress/plugin-markdown-ext";
import { markdownChartPlugin } from '@vuepress/plugin-markdown-chart'
import { searchPlugin } from '@vuepress/plugin-search'

export default defineUserConfig({
  title: "Zcash-Walletd Documentation",
  base: '/zcash-walletd/',
  bundler: viteBundler(),
  theme: defaultTheme({
    sidebar: [
      "/installation",
      "/usage",
    ],
  }),
  plugins: [
    searchPlugin({}),
    markdownImagePlugin({
      figure: true,
      lazyload: true,
      size: true,
    }),
    markdownMathPlugin({}),
    markdownExtPlugin({
      gfm: true,
      breaks: false,
    }),
    markdownChartPlugin({
      mermaid: true,
    }),
  ],
});
