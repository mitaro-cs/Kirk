<script lang="ts">
  let {
    checked,
    label,
    hint = '',
    onchange,
  }: { checked: boolean; label: string; hint?: string; onchange: (v: boolean) => void } = $props();
</script>

<label class="row">
  <span class="text">
    <span>{label}</span>
    {#if hint}<small>{hint}</small>{/if}
  </span>
  <input type="checkbox" role="switch" {checked} onchange={(e) => onchange(e.currentTarget.checked)} />
</label>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 0;
    cursor: pointer;
  }
  .row + :global(.row) {
    border-top: 1px solid var(--line);
  }
  .text {
    display: grid;
    gap: 2px;
  }
  small {
    color: var(--muted);
    font-size: 12px;
  }
  input {
    appearance: none;
    flex: none;
    width: 38px;
    height: 22px;
    border-radius: 999px;
    background: var(--off);
    position: relative;
    cursor: pointer;
    transition: background 200ms;
  }
  input::after {
    content: '';
    position: absolute;
    top: 2px;
    left: 2px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.3);
    transition: translate 200ms var(--ease-out);
  }
  input:checked {
    background: var(--accent);
  }
  input:checked::after {
    translate: 16px 0;
  }
  input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
