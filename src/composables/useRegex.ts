/**
 * Shared validation for the optional ticket / sub-group regex.
 *
 * The pattern is compiled in Rust, which degrades gracefully: an invalid pattern
 * is reported as a warning and the rule simply yields no sub-groups, so a typo
 * would otherwise look like it worked. Checking it in the browser is what makes
 * the mistake visible where it was made.
 *
 * NOTE: JavaScript and Rust regex syntax are not identical. This catches the
 * common typos (unbalanced groups/brackets, bad quantifiers) but a pattern can
 * still pass here and fail in Rust, which then surfaces as a warning.
 */
export function regexError(pattern: string | null | undefined): string | null {
  const trimmed = (pattern ?? '').trim()

  if (!trimmed) {
    return null
  }

  try {
    // Compiling is the whole check; an invalid pattern throws.
    new RegExp(trimmed)

    return null
  } catch (e) {
    return e instanceof Error ? e.message : 'Invalid regular expression'
  }
}
