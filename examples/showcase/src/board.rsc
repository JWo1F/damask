<div class="board">
  {use crate::notice::Notice}
  {use crate::tagged::Tagged}
  {use crate::theme::Theme}
  <Notice title="Deploy finished"/>
  <Notice title="Rollback" detail="check {self.log}" tone="warn" dismissible/>
  <Tagged value={42}/>
  <Theme/>
  <Theme label="Compact" dense={true}/>
</div>