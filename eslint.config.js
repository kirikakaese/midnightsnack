// SPDX-License-Identifier: GPL-3.0-or-later
import js from "@eslint/js";
import prettier from "eslint-config-prettier";
import svelte from "eslint-plugin-svelte";
import globals from "globals";
import ts from "typescript-eslint";

export default ts.config(
  {
    ignores: [
      "**/node_modules/",
      "**/dist/",
      "**/target/",
      "apps/host/src-tauri/gen/",
      "packages/protocol/src/generated/",
      "**/test-results/",
      "**/playwright-report/",
    ],
  },
  js.configs.recommended,
  ...ts.configs.recommended,
  ...svelte.configs.recommended,
  prettier,
  ...svelte.configs.prettier,
  {
    languageOptions: {
      globals: { ...globals.browser, ...globals.node },
    },
  },
  {
    files: ["**/*.svelte", "**/*.svelte.ts"],
    languageOptions: {
      parserOptions: { parser: ts.parser },
    },
  },
);
