<script lang="ts">
  interface Props {
    size?: number;
    'aria-hidden'?: boolean | 'true' | 'false';
  }
  let { size = 13, 'aria-hidden': ariaHidden }: Props = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="-8 -8 120 128"
  fill="none"
  stroke="currentColor"
  stroke-width="7"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden={ariaHidden}
>
  <rect x="34" y="0" width="36" height="26" rx="4" />
  <rect x="0" y="86" width="36" height="26" rx="4" />
  <rect x="68" y="86" width="36" height="26" rx="4" />
  <path d="M52,26 L52,55 L18,55 L18,86" />
  <path d="M52,55 L86,55 L86,86" />
</svg>
