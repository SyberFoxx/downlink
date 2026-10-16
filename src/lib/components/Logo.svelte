<script lang="ts">
  interface Props {
    /** Lado del cuadrado de marca, en píxeles. */
    size?: number;
    class?: string;
  }

  let { size = 24, class: className = "" }: Props = $props();
</script>

<!--
  Marca de Downlink: flecha descendente entrando en una bandeja, sobre un
  cuadrado con el degradado violeta→índigo de la app. Los colores son fijos a
  propósito (identidad de marca), no siguen al tema elegido.
-->
<svg
  viewBox="0 0 1024 1024"
  width={size}
  height={size}
  class="{className} shrink-0"
  aria-hidden="true"
  focusable="false"
>
  <defs>
    <linearGradient
      id="downlink-brand"
      x1="164"
      y1="120"
      x2="880"
      y2="912"
      gradientUnits="userSpaceOnUse"
    >
      <stop offset="0" stop-color="#8b5cf6" />
      <stop offset="0.52" stop-color="#7c3aed" />
      <stop offset="1" stop-color="#4338ca" />
    </linearGradient>
  </defs>
  <rect x="52" y="52" width="920" height="920" rx="212" fill="url(#downlink-brand)" />
  <g fill="none" stroke="#ffffff" stroke-width="92" stroke-linecap="round" stroke-linejoin="round">
    <path d="M512 268v352" />
    <path d="m388 500 124 124 124-124" />
    <path d="M296 748h432" />
  </g>
</svg>
