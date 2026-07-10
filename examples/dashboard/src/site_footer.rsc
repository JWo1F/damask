<footer>
<div class="wrap row">
<span>{self.fleet.services.len()} services · mean uptime {self.fleet.avg_uptime_label()} · target {self.fleet.slo_label()}</span>
<span>© {self.year} helm · build {self.short_commit()}</span>
</div>
</footer>
