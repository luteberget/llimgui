use rolling::input::staticinfrastructure as rolling_inf;
pub use rolling::output::history::History;

use crate::document::model::*;
use crate::document::interlocking::*;

pub fn get_history<'a>(vehicles :&ImShortGenList<Vehicle>, 
                   inf :&rolling_inf::StaticInfrastructure, 
                   il :&Interlocking,
                   commands :&[(f64, Command)]) -> Result<History, String> {

    // infrastructure and routes are already prepared by the dgraph module
    // we only need to convert commands to the rolling dispatch structs
    // and back from rolling history to glrail history

    use rolling::input::dispatch::DispatchAction;

    let mut dispatch = Vec::new();
    let mut t0 = 0.0;
    let mut train_no = 0;
    for (t,c) in commands {
        if *t > t0 {
            dispatch.push(DispatchAction::Wait(Some((t-t0) as _ )));
            t0 = *t;
        }

        match c {
            Command::Route(routespec) => {
                if let Some(route_idx) = il.find_route(routespec) {
                    dispatch.push(DispatchAction::Route(*route_idx));
                }
            }
            Command::Train(vehicle, routespec) => {
                if let Some(route_idx) = il.find_route(routespec) {
                    // get train params
                    let vehicle = vehicles.get(*vehicle).cloned().unwrap_or(Vehicle {
                        name :format!("Default train"),
                        length: 210.0,
                        max_acc: 0.95,
                        max_brk: 0.75,
                        max_vel: 180.0 / 3.6, // 180 km/h in m/s
                    });

                    let train_params = rolling::railway::dynamics::TrainParams {
                        length: vehicle.length as _,
                        max_acc: vehicle.max_acc as _,
                        max_brk: vehicle.max_brk as _,
                        max_vel: vehicle.max_vel as _,
                    };

                    // just make some name for now
                    let name = format!("train{}", train_no+1);
                    train_no += 1;

                    dispatch.push(DispatchAction::Train(name, train_params, *route_idx));
                }
            },
        }
    }

    //println!("Dispatch converted: {:#?}", dispatch);
    //println!(" Running rolling with");
    //println!("infrastructuer : {:?}", inf);
    //println!("routes : {:?}", routes);

    // TODO don't convert on the fly?
    //println!("Starting rolling");
    let history = rolling::evaluate_plan(inf,
                                         &il.routes.iter().map(|r| r.route.clone()).enumerate().collect(),
                                         &rolling::input::dispatch::Dispatch { actions: dispatch }, None);

    //println!("History output: {:?}", history);
    // TODO Convert back? Or just keep it like this
    //unimplemented!();

    Ok(history)
}