//! Scaffolds a slice's skeleton before its turn: the stem's crate, the part, its tests, and its lines in every list.

mod brief;
mod lists;
mod names;
mod reference;
mod render;
mod stem;
mod trigger;
mod types;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use emery_sdk::target::{DesignType, Guarded, Marker, Requirement, Slice};
use emery_sdk::{Error, Standing, push_unique, server_error, unique, write_whole};

use self::lists::{Lists, Placement, Topics};
use self::reference::{Idiom, Role};
use self::stem::{Kind, Stem, lines_of};
use self::types::DesignKey;
use crate::layout;

// A slice's skeleton as read against the tree and not yet written: each
// stem's files, held or to scaffold, and each line placed in its list.
pub struct Plan {
    stems: Vec<Stem>,
    start: Vec<String>,
    relaid: Vec<Relaid>,
    existing: BTreeSet<String>,
    lists: Lists,
    roles: BTreeSet<Role>,
    idioms: BTreeSet<Idiom>,
}

// A slice's skeleton as written, which the turn fills and the check holds
// the tree to.
pub struct Scaffold {
    plan: Plan,
    written: Vec<String>,
    listing: BTreeSet<String>,
}

// What a check reads of one scenario the scaffold wrote a test for.
pub struct ExpectedTest<'a> {
    pub path: &'a str,
    pub id: &'a str,
    pub scenario: &'a str,
    pub test: &'a str,
}

// One stem's share of the slice before its files are planned.
struct StemPlan<'s> {
    stem: Stem,
    requirements: Vec<&'s Requirement>,
    owned: Vec<&'s DesignType>,
}

// A `start` requirement laid under the crate of its route's resource.
struct Relaid {
    id: String,
    route: String,
    stem: String,
}

impl Scaffold {
    pub fn plan(slice: &Slice, root: &Path) -> Result<Plan, Error> {
        let existing: BTreeSet<String> = layout::crates(root)?.into_iter().collect();
        let mut topics = Topics::read(root)?;
        let design_keys = types::design_keys(&slice.design);
        let idioms = reference::idioms(slice);
        let roles = reference::roles(slice);
        let probing = idioms.contains(&Idiom::Probe);
        let sql = probing || roles.contains(&Role::Table);

        // each requirement to its stem: `start` set aside, unless a typed
        // route names the resource whose crate serves it
        let mut plans: Vec<StemPlan> = Vec::new();
        let mut start = Vec::new();
        let mut relaid = Vec::new();
        for requirement in &slice.requirements {
            let own = requirement.stem();
            let name = if layout::is_start_stem(own) {
                let resource = trigger::route(requirement)
                    .map(|route| (route, trigger::resource(route)))
                    .and_then(|(route, stem)| Some((route, stem?)))
                    .filter(|(_, stem)| !layout::is_start_stem(stem));
                let Some((route, stem)) = resource else {
                    start.push(requirement.id.clone());
                    continue;
                };
                relaid.push(Relaid {
                    id: requirement.id.clone(),
                    route: route.to_string(),
                    stem: stem.clone(),
                });
                stem
            } else {
                own.to_owned()
            };
            match plans.iter_mut().find(|plan| plan.stem.name == name) {
                Some(plan) => plan.requirements.push(requirement),
                None => plans.push(StemPlan {
                    stem: Stem::new(&name, slice, probing, sql),
                    requirements: vec![requirement],
                    owned: Vec::new(),
                }),
            }
        }

        // each owned type to the stem its key leads with, else the first
        for design_type in &slice.types {
            let stem = DesignKey::parse(&design_type.key).stem;
            let at = plans
                .iter()
                .position(|plan| Some(plan.stem.name.as_str()) == stem)
                .or_else(|| (!plans.is_empty()).then_some(0));
            if let Some(at) = at {
                plans[at].owned.push(design_type);
            }
        }

        let mut lists = Lists::default();
        let mut stems = Vec::new();
        for plan in plans {
            let mut stem = plan.stem;
            stem.plan(&plan.owned, &plan.requirements, &design_keys, &mut topics);
            stem.read(root)?;
            stem.place_lines(root, &mut lists)?;
            stems.push(stem);
        }
        Ok(Plan {
            stems,
            start,
            relaid,
            existing,
            lists,
            roles,
            idioms,
        })
    }

    // Every file the scaffold wrote and every list it added a line to.
    pub fn written(&self) -> &[String] {
        &self.written
    }

    // Every file the scaffold wrote or found and every list it holds a line
    // in, each with what a whole write of it must keep, sorted by path: a
    // module its requirement ids and the items the scaffold named, a test
    // file its ids and its tests, and a manifest, an entry, or a list every
    // line it holds, so a list is added to and never cut; and, for a module
    // or a test file the scaffold wrote, each marker it laid under the key
    // the brief quotes, so the turn fills a marker by naming it.
    fn guarded(&self) -> Vec<Guarded> {
        let mut keeps: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        let mut markers: BTreeMap<&str, Vec<Marker>> = BTreeMap::new();
        for stem in &self.plan.stems {
            for kind in Kind::ALL {
                let Some(file) = stem.file(kind) else { continue };
                keeps.entry(&file.path).or_default().extend(stem.keeps(kind));
                markers.entry(&file.path).or_default().extend(stem.markers(kind));
            }
            for line in stem.lines.iter().filter(|line| line.placement != Placement::Unanchored) {
                let tokens =
                    self.plan.lists.get(&line.path).map_or_else(|| lines_of(&line.text), lines_of);
                keeps.entry(&line.path).or_default().extend(tokens);
            }
        }
        keeps
            .into_iter()
            .map(|(path, tokens)| {
                Guarded::new(path, unique(tokens))
                    .with_markers(markers.remove(path).unwrap_or_default())
            })
            .collect()
    }

    // The design keys of the types laid into the modules the scaffold wrote,
    // a marker among them, since its signature stands fenced in its doc; a
    // held module holds whatever an earlier slice left, so its types are
    // none.
    fn types(&self) -> Vec<String> {
        self.plan
            .stems
            .iter()
            .filter(|stem| !stem.module.held)
            .flat_map(|stem| stem.types.iter().map(|design_type| design_type.key.clone()))
            .collect()
    }

    // The files of the tree once the skeleton was written, which the check
    // holds the tree to: a file beyond them outside the slice's crates is a
    // stray.
    pub const fn listing(&self) -> &BTreeSet<String> {
        &self.listing
    }

    pub fn stems(&self) -> impl Iterator<Item = &str> {
        self.plan.stems.iter().map(|stem| stem.name.as_str())
    }

    pub const fn existing(&self) -> &BTreeSet<String> {
        &self.plan.existing
    }

    pub fn expected_tests(&self) -> impl Iterator<Item = ExpectedTest<'_>> {
        self.plan
            .stems
            .iter()
            .filter_map(|stem| stem.tests.as_ref().map(|file| (stem, file)))
            .flat_map(|(stem, file)| {
                stem.scenarios.iter().map(move |test| ExpectedTest {
                    path: &file.path,
                    id: &test.id,
                    scenario: &test.scenario.name,
                    test: &test.name,
                })
            })
    }
}

impl Plan {
    // A slice of `start` requirements alone, none carrying a route, lays no
    // stem: nothing is scaffolded and no turn is opened for it.
    pub const fn is_empty(&self) -> bool {
        self.stems.is_empty()
    }

    // Writes each stem's files the tree lacks and adds each placed line to
    // its list, every list written once with its lines in order.
    pub fn write(mut self, root: &Path) -> Result<Scaffold, Error> {
        let mut written = Vec::new();
        for stem in &self.stems {
            for kind in Kind::ALL {
                if let Some(file) = stem.file(kind).filter(|file| !file.held) {
                    create(root, &file.path, &stem.render(kind))?;
                    written.push(file.path.clone());
                }
            }
        }

        let mut touched: Vec<&str> = Vec::new();
        for line in self.stems.iter().flat_map(|stem| &stem.lines) {
            if line.placement == Placement::Add && self.lists.add(line) {
                push_unique(&mut touched, &line.path);
            }
        }
        for path in touched {
            let Some(text) = self.lists.get(path) else { continue };
            write_whole(root, path, Standing::File, text.as_bytes())
                .map_err(|err| server_error!("writing `{path}`: {err}"))?;
            written.push(path.to_owned());
        }
        let listing = layout::listing(root)?;
        Ok(Scaffold {
            plan: self,
            written,
            listing,
        })
    }
}

// Creates the file the plan found absent, its directories made one at a
// time and no link followed.
fn create(root: &Path, path: &str, text: &str) -> Result<(), Error> {
    write_whole(root, path, Standing::Absent, text.as_bytes())
        .map_err(|err| server_error!("creating `{path}`: {err}"))
}
