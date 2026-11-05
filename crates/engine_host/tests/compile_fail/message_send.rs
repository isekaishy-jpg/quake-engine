use std::rc::Rc;
use engine_host::{Application, Context, Error, StopProgress};
struct App;
impl Application for App {
    type Message = Rc<()>;
    type LocalEvent = ();
    fn started(&mut self, _: &mut Context<'_, Self>) -> Result<(), Error> { Ok(()) }
    fn stopping(&mut self, _: &mut Context<'_, Self>) -> StopProgress { StopProgress::Settled }
}
fn main() {}
