use log::*;
use std::sync::mpsc::*;
use std::collections::HashMap;

use crate::document::model::*;
use crate::document::dgraph::*;
use crate::document::topology;
use crate::document::interlocking;
use crate::document::infview::unround_coord;

use crate::document::history;
use crate::app;
use crate::util;
use crate::util::VecMap;
use crate::document::dispatch;
use crate::document::plan;
use std::sync::Arc;
use nalgebra_glm as glm;

#[derive(Default)]
pub struct Derived {
    pub dgraph :Option<Arc<DGraph>>,
    pub interlocking :Option<Arc<interlocking::Interlocking>>,
    pub topology: Option<Arc<topology::Topology>>,
    pub dispatch :Vec<Option<dispatch::DispatchView>>,
    pub plandispatches :HashMap<usize, Vec<Option<dispatch::DispatchView>>>,
}

pub struct ViewModel {
    pub(super) model: Undoable<Model, EditClass>,
    pub(super) derived :Derived,
    get_data :Option<Receiver<SetData>>,
    bg :app::BackgroundJobs,
}

#[derive(Debug)]
pub enum SetData {
    DGraph(Arc<DGraph>),
    Interlocking(Arc<interlocking::Interlocking>),
    Dispatch(usize,dispatch::DispatchView),
    PlanDispatch(usize,usize,dispatch::DispatchView),
}

impl app::BackgroundUpdates for ViewModel {
    fn check(&mut self) {
        while let Some(Ok(data)) = self.get_data.as_mut().map(|r| r.try_recv()) {
            match data {
                SetData::DGraph(dgraph) => { self.derived.dgraph = Some(dgraph); },
                SetData::Interlocking(il) => { self.derived.interlocking = Some(il); },
                SetData::Dispatch(idx,h) => { 
                    self.derived.dispatch.vecmap_insert(idx,h);
                    //cache.clear_dispatch(idx);
                },
                SetData::PlanDispatch(plan_idx,dispatch_idx,h) => {
                    self.derived.plandispatches.entry(plan_idx)
                        .or_insert(Vec::new())
                        .vecmap_insert(dispatch_idx, h);
                },
            }
        }
    }
}

impl ViewModel {
    pub fn model(&self) -> &Model { &self.model.get() }
    pub fn data(&self) -> &Derived { &self.derived }
    pub fn from_model(model :Model, bg: app::BackgroundJobs) -> Self {
        ViewModel {
            model: Undoable::from(model),
            derived: Default::default(),
            get_data: None,
            bg: bg,
        }
    }

    pub(super) fn update(&mut self) {
        let model = self.model.get().clone(); // persistent structs
        let topology = Arc::new(topology::convert(&model, 50.0).unwrap());
        self.derived.topology = Some(topology.clone());

        let (tx,rx) = channel();
        self.get_data = Some(rx);
        self.bg.execute(move || {
            info!("Background thread starting");
            let model = model;  // move model into thread
            let tx = tx;        // move sender into thread

            //let dgraph = dgraph::calc(&model); // calc dgraph from model.
            let dgraph = DGraphBuilder::convert(&model,&topology).expect("dgraph conversion failed");
            let dgraph = Arc::new(dgraph);

            info!("Dgraph successful with {:?} nodes", dgraph.rolling_inf.nodes.len());

            let send_ok = tx.send(SetData::DGraph(dgraph.clone()));
            if !send_ok.is_ok() { println!("job canceled after dgraph"); return; }
            // if tx fails (channel is closed), we don't need 
            // to proceed to next step. Also, there is no harm
            // in *trying* to send the data from an obsolete thread,
            // because the update function will have replaced its 
            // receiver end of the channel, so it will anyway not
            // be placed into the struct.

            let interlocking = interlocking::calc(&dgraph); 
            let interlocking = Arc::new(interlocking);
                // calc interlocking from dgraph
            let send_ok = tx.send(SetData::Interlocking(interlocking.clone()));
            if !send_ok.is_ok() { println!("job canceled after interlocking"); return; }
            info!("Interlocking successful with {:?} routes", interlocking.routes.len());

            for (i,dispatch) in model.dispatches.iter() {
                //let history = dispatch::run(&dgraph, &interlocking, &dispatch);
                let history = history::get_history(&model.vehicles,
                                                   &dgraph.rolling_inf,
                                                   &interlocking,
                                                   &(dispatch.0)).unwrap();
                info!("Simulation successful {:?}", &dispatch.0);
                let view = dispatch::DispatchView::from_history(&dgraph, history);
                let send_ok = tx.send(SetData::Dispatch(*i, view));
                if !send_ok.is_ok() { println!("job canceled after dispatch"); return; }
            }

            for (plan_idx,plan) in model.plans.iter() {
                let dispatches = plan::get_dispatches(&interlocking,
                                             &model.vehicles,
                                             plan).unwrap();

                info!("Planninc successful. {:?}", dispatches);

                for (dispatch_idx,d) in dispatches.into_iter().enumerate() {
                    let history = history::get_history(&model.vehicles,
                                         &dgraph.rolling_inf,
                                         &interlocking,
                                         &d.0).unwrap(); // TODO UNWRAP?
                    info!("Planned simulation successful");
                    let view = dispatch::DispatchView::from_history(&dgraph, history);
                    let send_ok = tx.send(SetData::PlanDispatch(*plan_idx, dispatch_idx, view));
                    if !send_ok.is_ok() { println!("job cancelled after plan dispatch {}/{}", 
                                                   plan_idx, dispatch_idx); }
                }

            }

        });
    }

}
