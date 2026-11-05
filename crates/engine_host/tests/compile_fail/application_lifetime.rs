use engine_host::{Application, Context, Error, StopProgress};
struct App<'data>(&'data u8);
impl Application for App<'_> {
    type Message = String;
    type LocalEvent = u8;
    fn started(&mut self, _: &mut Context<'_, Self>) -> Result<(), Error> { Ok(()) }
    fn stopping(&mut self, _: &mut Context<'_, Self>) -> StopProgress { StopProgress::Settled }
}
fn change<'cx, 'short>(cx: Context<'cx, App<'static>>) -> Context<'cx, App<'short>> { cx }
fn main() {}
