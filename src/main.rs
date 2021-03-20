use anyhow::Result;
use clap_verbosity_flag::Verbosity;
use serde::{Deserialize, Serialize};
use simplelog::{ConfigBuilder, TermLogger, TerminalMode};
use std::collections::BTreeMap;
use std::{
    fs::File,
    io::{stdin, Read},
    path::PathBuf,
};
use structopt::StructOpt;

#[macro_use]
extern crate log;

#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(flatten)]
    verbose: Verbosity,

    #[structopt(long, short, parse(from_os_str))]
    input: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Clone)]
struct Thing {
    name: String,
    value: f64,
    num: usize,
    costs: Vec<usize>,
}

#[derive(Debug, Serialize)]
struct TakedThing {
    name: String,
    num: usize,
}

#[derive(Debug, Deserialize)]
struct UncheckedProblem {
    #[serde(alias = "Things")]
    things: Vec<Thing>,
    costs: Vec<usize>,
}

impl UncheckedProblem {
    fn check(self) -> Result<Problem> {
        let len = self.costs.len();
        if len == 0 {
            anyhow::bail!("must contain at least one cost");
        }
        if self.things.iter().all(|thing| thing.costs.len() == len) {
            Ok(Problem::new(self.things, self.costs))
        } else {
            anyhow::bail!("costs does not match");
        }
    }
}

#[derive(Debug)]
struct Costs(Vec<usize>);

impl Costs {
    fn end(&self) -> usize {
        self.to_idx(&self.0)
    }

    fn iter(&self) -> std::ops::RangeInclusive<usize> {
        0..=self.end()
    }

    fn to_idx(&self, vec: &[usize]) -> usize {
        let mut ans = 0;
        for (idx, c) in self.0.iter().skip(1).enumerate() {
            ans += vec[idx];
            ans *= c + 1;
        }
        ans + *vec.last().unwrap() as usize
    }

    fn to_cost(&self, mut c: usize) -> Vec<usize> {
        let mut costs = Vec::new();
        for bound in self.0.iter().rev() {
            let idx = c % (bound + 1);
            c /= bound + 1;
            costs.push(idx);
        }
        costs.reverse();
        costs
    }

    fn validate_sub(&self, bound: &[usize], cost: &[usize]) -> Option<usize> {
        let mut ans = 0;
        for idx in 0..bound.len() {
            if cost[idx] > bound[idx] {
                return None;
            } else {
                let c = if idx + 1 < self.0.len() {
                    self.0[idx + 1]
                } else {
                    0
                };
                ans += bound[idx] - cost[idx];
                ans *= c + 1;
            }
        }
        Some(ans)
    }
}

#[derive(Debug)]
struct Problem {
    things: Vec<Thing>,
    costs: Costs,
    dp: Vec<f64>,
}

impl Problem {
    fn new(things: Vec<Thing>, costs: Vec<usize>) -> Self {
        let costs = Costs(costs);
        let dp = vec![0.0; costs.end() + 1];

        Self { things, costs, dp }
    }

    fn zero_one_pack(&mut self, cost: &[usize], value: f64, k: usize, taked: &mut Vec<usize>) {
        for c in self.costs.iter().rev() {
            let bound = self.costs.to_cost(c);
            if let Some(idx) = self.costs.validate_sub(&bound, cost) {
                let v = self.dp[idx] + value;
                if v > self.dp[c] {
                    self.dp[c] = v;
                    taked[c] = taked[idx] + k;
                }
            }
        }
    }

    fn multi_pack(&mut self, cost: &[usize], value: f64, mut num: usize) -> Vec<usize> {
        let mut k = 1;
        let mut taked = vec![0; self.costs.end() + 1];
        while k < num {
            self.zero_one_pack(
                &cost.iter().map(|c| c * k as usize).collect::<Vec<_>>(),
                k as f64 * value,
                k,
                &mut taked,
            );
            num -= k;
            k *= 2;
        }
        if num > 0 {
            let k = num;
            self.zero_one_pack(
                &cost.iter().map(|c| c * k as usize).collect::<Vec<_>>(),
                k as f64 * value,
                k,
                &mut taked,
            );
        }

        taked
    }

    fn solve(mut self) -> Solution {
        let mut taked = Vec::new();
        let mut chosen = Vec::new();
        for thing in self.things.clone() {
            taked.push(self.multi_pack(&thing.costs, thing.value, thing.num));
        }
        let mut v = self.costs.end();
        for k in (0..self.things.len()).rev() {
            let num = taked[k][v];
            chosen.push(num);
            v -= self.costs.to_idx(
                &self.things[k]
                    .costs
                    .iter()
                    .map(|c| *c * num as usize)
                    .collect::<Vec<_>>(),
            );
        }
        chosen.reverse();
        let chosen = chosen
            .iter()
            .enumerate()
            .map(|(idx, num)| (self.things[idx].name.clone(), *num))
            .collect();
        Solution {
            value: self.dp[self.costs.end()],
            chosen,
        }
    }
}

#[derive(Debug, Serialize)]
struct Solution {
    value: f64,
    chosen: BTreeMap<String, usize>,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();
    let log_config = ConfigBuilder::new().build();
    if let Some(level) = opt.verbose.log_level() {
        TermLogger::init(level.to_level_filter(), log_config, TerminalMode::Mixed)?;
    }
    debug!("opt={:?}", opt);
    let mut buf = String::new();
    if let Some(path) = &opt.input {
        let mut input_file = File::open(path)?;
        input_file.read_to_string(&mut buf)?;
    } else {
        let mut stdin = stdin();
        while let Ok(n_bytes) = stdin.read_to_string(&mut buf) {
            if n_bytes == 0 {
                break;
            }
        }
    }

    let solution = toml::from_str::<UncheckedProblem>(&buf)?.check()?.solve();
    print!("{}", toml::to_string(&solution)?);
    Ok(())
}
