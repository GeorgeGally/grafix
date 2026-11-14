<script lang="ts">
  import { Handle, Position, type NodeProps } from '@xyflow/svelte';
  import type { GraphCardData, PortType } from './graph-types';

  let { data, selected = false }: NodeProps = $props();
  const spec = $derived((data as GraphCardData).spec);
  const rows = $derived(Math.max(spec.inputs.length, spec.outputs.length, 1));
  const colors: Record<PortType, string> = {
    geometry: '#dcb482', scene_object: '#dcb482', instance_set: '#e8b27d',
    texture2_d: '#96b4ea', light: '#f1df9d', scene: '#ced7eb',
    camera: '#c6b7fa', effect: '#a5cfba', float: '#b6c3ce',
  };
</script>
<div class:selected class="node" style:--rows={rows}>
  <div class="head">
    <span>{spec.category}</span>
    <strong>{spec.title}</strong>
  </div>
  <div class="ports">
    <div>
      {#each spec.inputs as input, i (input.name)}
        <div class="port">
          <Handle type="target" id={input.name} position={Position.Left}
            style={`top: ${51 + i * 23}px; left:-6px; background:${colors[input.data_type]}; border:2px solid #111419; width:11px; height:11px;`}/>
          <span>{input.name}</span>
        </div>
      {/each}
    </div>
    <div>
      {#each spec.outputs as output, i (output.name)}
        <div class="port output">
          <span>{output.name}</span>
          <Handle type="source" id={output.name} position={Position.Right}
            style={`top: ${51 + i * 23}px; right:-6px; background:${colors[output.data_type]}; border:2px solid #111419; width:11px; height:11px;`}/>
        </div>
      {/each}
    </div>
  </div>
</div>
<style>
.node { width:185px; min-height:calc(51px + var(--rows) * 23px + 9px); background:#171b23; border:1px solid #404957; border-radius:7px; box-shadow:0 7px 28px rgb(0 0 0/.22); color:#dce2ec; font:11px/1.35 var(--mono); }
.node.selected { border-color:#82c8f4; box-shadow:0 0 0 1px rgb(130 200 244/.4),0 7px 28px rgb(0 0 0/.25); }
.head { height:43px; display:flex; flex-direction:column; justify-content:center; gap:2px; padding:5px 12px; background:#252a34; border-radius:6px 6px 0 0; }
.head strong { font-size:12px; font-weight:600; }
.head span { color:#8995a6; font-size:9px; text-transform:uppercase; letter-spacing:.09em; }
.ports { display:grid; grid-template-columns:1fr 1fr; gap:4px; padding:7px 12px 9px; }
.port { height:23px; display:flex; align-items:center; color:#aab6c7; white-space:nowrap; }
.output { justify-content:flex-end; }
</style>
