<section>
<h1>Recent deploys</h1>
{#if self.deploys.is_empty()}
<p class="empty">Nothing has shipped in the last 24 hours.</p>
{:else}
<ul class="feed">
{#each self.visible() as d}
<li>
<span class="svc">{d.service}</span>
<span class="ver">{d.version}</span>
<span class="owner">by {d.author}</span>
{#if d.rolled_back}<span class="rb">rolled back</span>{/if}
<span class="ago">{d.when()}</span>
</li>
{/each}
</ul>
{#if self.hidden() > 0}
<p class="sub">and {self.hidden()} older deploy(s) not shown.</p>
{/if}
{/if}
</section>
