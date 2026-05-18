import pluginVue from 'eslint-plugin-vue'
import tseslint from 'typescript-eslint'
import prettierConfig from 'eslint-config-prettier'

export default tseslint.config(
  {
    ignores: [
      '**/dist/**',
      '**/node_modules/**',
      '**/.wrangler/**',
      'api/drizzle/**',
      'api/scripts/**',
    ],
  },

  // TypeScript + JS recommended rules
  ...tseslint.configs.recommended,

  // Vue 3 recommended rules (.vue files)
  ...pluginVue.configs['flat/recommended'],

  // Use the TypeScript parser for <script lang="ts"> blocks inside .vue files
  {
    files: ['**/*.vue'],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser,
      },
    },
  },

  // Project-wide custom rules
  {
    rules: {
      curly: 'error',
      'prefer-const': 'error',
      'no-var': 'error',
      eqeqeq: ['error', 'always', { null: 'ignore' }],

      'padding-line-between-statements': [
        'error',
        // Blank line before return, except when it's the only statement in the block
        { blankLine: 'always', prev: '*', next: 'return' },
        { blankLine: 'any', prev: ['block-like', 'block'], next: 'return' },
        { blankLine: 'never', prev: 'empty', next: 'return' },
        // Blank line before return is optional when it's the sole statement
        { blankLine: 'any', prev: ['const', 'let', 'var', 'expression'], next: 'return' },
        { blankLine: 'always', prev: 'multiline-const', next: 'return' },
        { blankLine: 'always', prev: 'multiline-let', next: 'return' },
        // Blank line after the last import group
        { blankLine: 'always', prev: 'import', next: '*' },
        { blankLine: 'any', prev: 'import', next: 'import' },
        // Blank line before/after block statements (if, for, while, try, switch)
        { blankLine: 'always', prev: '*', next: 'block-like' },
        { blankLine: 'always', prev: 'block-like', next: '*' },
        // Allow adjacent block-like statements (e.g. if/else chains) without extra blank lines
        { blankLine: 'any', prev: 'block-like', next: 'block-like' },
        // Blank line before throw
        { blankLine: 'always', prev: '*', next: 'throw' },
        // Blank lines around function and class declarations
        { blankLine: 'always', prev: '*', next: ['function', 'class'] },
        { blankLine: 'always', prev: ['function', 'class'], next: '*' },
      ],

      // Enforce `import { type Foo }` over a separate `import type` statement
      '@typescript-eslint/consistent-type-imports': [
        'error',
        { prefer: 'type-imports', fixStyle: 'inline-type-imports' },
      ],

      // Flag unused vars/args; underscore-prefix opt-out convention
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_', ignoreRestSiblings: true },
      ],

      // Warn on `any` rather than hard-blocking (pragmatic for a complex data project)
      '@typescript-eslint/no-explicit-any': 'warn',
    },
  },

  // Prettier — disable all ESLint rules that would conflict with Prettier's formatting
  prettierConfig,

  // Re-assert rules that eslint-config-prettier unconditionally disables but we still want
  {
    rules: {
      curly: 'error',
    },
  },
)
