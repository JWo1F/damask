<header class="masthead">
<div class="wrap">
{use crate::model::Status}
<div class="masthead-row">
<div class="brand">helm <span>/ fleet status</span></div>
<nav>
{#each &self.nav as entry}
<a href={self.href(entry)} class={self.nav_class(entry)}>{entry}</a>
{/each}
</nav>
</div>
{#if self.fleet.all_clear()}
<p class="banner ok">All {self.fleet.services.len()} services are healthy and meeting the {self.fleet.slo_label()} availability target.</p>
{:else if self.fleet.worst() == Status::Down}
<p class="banner alert">{self.fleet.count(Status::Down)} service(s) down — {self.fleet.breaching()} of {self.fleet.services.len()} are below the {self.fleet.slo_label()} target.</p>
{:else}
<p class="banner alert">Degraded: {self.fleet.breaching()} of {self.fleet.services.len()} services are below the {self.fleet.slo_label()} target.</p>
{/if}
</div>
</header>
