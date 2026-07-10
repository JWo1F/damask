<table>
{use crate::status_badge::StatusBadge}
<thead>
<tr><th>Service</th><th>Status</th><th>Uptime</th><th>p95</th><th>Version</th></tr>
</thead>
<tbody>
{#each self.services as svc, i}
<tr class={self.row_class(svc, i)}>
<td><div class="svc">{svc.name}</div><div class="owner">{svc.owner}</div></td>
<td><StatusBadge status={svc.status}/></td>
<td>{svc.uptime()}</td>
<td>{svc.latency()}{#if svc.is_slow()}<span class="flag">slow</span>{/if}</td>
<td class="ver">{svc.version}</td>
</tr>
{/each}
</tbody>
</table>
