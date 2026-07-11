<section>
  {use crate::deploy_feed::DeployFeed}
  {use crate::model::Status}
  {use crate::service_table::ServiceTable}
  {#snippet tile(class, count, caption)}<div class={class}><div class="n">{count}</div><div class="k">{caption}</div></div>{/snippet}
  <h1>Fleet overview</h1>
  <p class="sub">{self.fleet.services.len()} services · mean uptime {self.fleet.avg_uptime_label()} · worst {self.fleet.worst()}</p>
  <div class="tiles">
    {@render tile("tile healthy", self.fleet.count(Status::Healthy), "healthy")}
    {@render tile("tile degraded", self.fleet.count(Status::Degraded), "degraded")}
    {@render tile("tile down", self.fleet.count(Status::Down), "down")}
    {@render tile("tile", self.fleet.breaching(), "below SLO")}
  </div>
  <ServiceTable services={&self.fleet.services} slo_target={self.fleet.slo_target}/>
  <DeployFeed deploys={&self.fleet.deploys} limit={self.feed_limit}/>
</section>
