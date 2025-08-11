import { defineConfig } from "vitepress";

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "Venice Program Table",
  description:
    "Multi-purpose file format for delivering code to VEX V5 programs ",
  themeConfig: {
    // https://vitepress.dev/reference/default-theme-config
    nav: [{ text: "Home", link: "/" }],

    sidebar: [
      {
        text: "Specification",
        items: [
          { text: "Introduction", link: "/introduction" },
          { text: "Advanced", link: "/advanced" },
        ],
      },
    ],

    socialLinks: [
      {
        icon: "github",
        link: "https://github.com/venice-v5/venice-program-table",
      },
    ],
  },
});
