<script lang="ts">
  interface Props {
    /** Lado del cuadrado de marca, en píxeles. */
    size?: number;
    class?: string;
  }

  let { size = 24, class: className = "" }: Props = $props();
</script>

<!--
  Marca de Downlink: la **imagen original** de la app (`static/brand/icon.svg`),
  la misma que usan el favicon y los iconos del sistema. Se carga el archivo en
  vez de duplicar el dibujo en código para que haya una sola fuente de verdad:
  si se cambia la marca, cambia en todos los sitios a la vez.

  Va con `object-contain` para respetar el cuadrado del icono, y sin sombra ni
  fondo, así encaja en cualquier sitio donde se ponga.
-->
<img
  src="/brand/icon.svg"
  width={size}
  height={size}
  alt="Downlink"
  class="{className} shrink-0 object-contain"
  draggable="false"
/>
