<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1"/>
    <title>{self.title}</title>
    <style>{@html crate::theme::CSS}</style>
  </head>
  <body>
    {use crate::site_footer::SiteFooter}
    {use crate::site_header::SiteHeader}
    <SiteHeader fleet={self.fleet} nav={self.nav.clone()} current={self.current}/>
    <main class="wrap"><slot/></main>
    <SiteFooter fleet={self.fleet} commit={self.commit.clone()} year={self.year}/>
  </body>
</html>
