use engine_host::{Application, Context, Error, StopProgress};
struct App;
impl Application for App {
    type Message = String;
    type LocalEvent = u8;
    fn started(&mut self, _: &mut Context<'_, Self>) -> Result<(), Error> { Ok(()) }
    fn stopping(&mut self, _: &mut Context<'_, Self>) -> StopProgress { StopProgress::Settled }
}
fn escape<'cx>(cx: Context<'cx, App>) -> Context<'static, App> { cx }
fn main() {}
