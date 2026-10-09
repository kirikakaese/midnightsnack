<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";

  type Variant = "default" | "go" | "danger" | "freeze" | "logo" | "ghost";
  type Size = "md" | "lg" | "xl";

  interface Props extends HTMLButtonAttributes {
    variant?: Variant;
    size?: Size;
    /** Renders the button as latched (e.g. blackout is active). */
    active?: boolean;
    children: Snippet;
  }

  let {
    variant = "default",
    size = "md",
    active = false,
    type = "button",
    class: className = "",
    children,
    ...rest
  }: Props = $props();
</script>

<button
  class="ms-btn ms-btn--{variant} ms-btn--{size} {className}"
  class:ms-btn--active={active}
  aria-pressed={variant === "default" || variant === "ghost" ? undefined : active}
  {type}
  {...rest}
>
  {@render children()}
</button>

<style>
  .ms-btn {
    min-height: var(--ms-touch);
    min-width: var(--ms-touch);
    padding: 0 18px;
    border: 2px solid var(--ms-border);
    border-radius: var(--ms-radius);
    background: var(--ms-surface-2);
    color: var(--ms-text);
    font: inherit;
    font-weight: 600;
    letter-spacing: 0.02em;
    cursor: pointer;
    user-select: none;
    touch-action: manipulation;
    transition:
      background-color 80ms linear,
      border-color 80ms linear;
  }
  /* Touchscreens keep :hover after a tap, so only real pointers get a hover state, and it
     never hides a latched (active) state. */
  @media (hover: hover) {
    .ms-btn:hover:not(:disabled):not(.ms-btn--active) {
      background: var(--ms-surface-3);
    }
  }
  .ms-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .ms-btn--lg {
    min-height: 64px;
    font-size: 1.15rem;
  }
  .ms-btn--xl {
    min-height: 96px;
    font-size: 1.6rem;
  }
  .ms-btn--default.ms-btn--active,
  .ms-btn--ghost.ms-btn--active {
    border-color: var(--ms-accent);
    box-shadow: inset 0 0 0 1px var(--ms-accent);
  }
  .ms-btn--ghost {
    background: transparent;
    border-color: transparent;
  }
  .ms-btn--go {
    border-color: var(--ms-go);
  }
  .ms-btn--go.ms-btn--active,
  .ms-btn--go:active {
    background: var(--ms-go);
    color: var(--ms-go-text);
  }
  .ms-btn--danger {
    border-color: var(--ms-danger);
  }
  .ms-btn--danger.ms-btn--active {
    background: var(--ms-danger);
    color: #fff;
  }
  .ms-btn--freeze {
    border-color: var(--ms-freeze);
  }
  .ms-btn--freeze.ms-btn--active {
    background: var(--ms-freeze);
    color: #04202c;
  }
  .ms-btn--logo {
    border-color: var(--ms-logo);
  }
  .ms-btn--logo.ms-btn--active {
    background: var(--ms-logo);
    color: #1b0f3a;
  }
</style>
