use starlight::options::Options;
use starlight::vm::context::Context;
use starlight::Platform;
use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    //Platform::initialize();
    let todos = Rc::new(RefCell::new(vec![]));
    let todos2 = todos.clone();
    let options = Options::default();
    println!("starting");
    let mut starlight_runtime =
        Platform::new_runtime(options, None).with_async_scheduler(Box::new(move |job| {
            println!("sched job");
            todos2.borrow_mut().push(job);
        }));
    let mut ctx = Context::new(&mut starlight_runtime);

    ctx.heap().gc();

    match ctx
        .eval("let p = Promise.all([new Promise((resA, rejA) => {resA(123);}), new Promise((resB, rejB) => {resB(456);})]); p.then((res) => {print('pAll resolved to ' + res.join(','));}); p.catch((res) => {print('pAll rejected to ' + res);});")
    {
        Ok(_) => {

            println!("prom code running");
        }
        Err(e) => {
            println!("{:?}",e);
        }
    }
}
