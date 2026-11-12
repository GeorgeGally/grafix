<script lang="ts">
  interface Props {
    label: string;
    value: number;
    min: number;
    max: number;
    step: number;
    format?: (value: number) => string;
    disabled?: boolean;
    oninput: (value: number) => void;
  }
  let {
    label,
    value = $bindable(),
    min,
    max,
    step,
    format = (v) => v.toFixed(2),
    disabled = false,
    oninput,
  }: Props = $props();
  const id = `slider-${Math.random().toString(36).slice(2)}`;
</script>

<div class="slider">
  <label for={id}>{label}</label>
  <output for={id}>{format(value)}</output>
  <input
    {id}
    type="range"
    {min}
    {max}
    {step}
    {disabled}
    bind:value
    oninput={() => oninput(value)}
    onchange={() => oninput(value)}
  />
</div>

<style>
  .slider {
    display: grid;
    grid-template-columns: 1fr auto;
    row-gap: 4px;
  }
  label { color: var(--text); }
  output { color: var(--muted); font-family: var(--mono); font-size: 12px; }
  input {
    grid-column: 1 / -1;
    width: 100%;
    margin: 0;
    accent-color: var(--accent);
  }
</style>
