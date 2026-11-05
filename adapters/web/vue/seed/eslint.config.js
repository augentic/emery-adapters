import pluginVue from "eslint-plugin-vue";

export default [
  ...pluginVue.configs["flat/recommended"],
  { files: ["src/Page.vue"], rules: { "vue/multi-word-component-names": "off" } },
];
