use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use syn::visit::{self, Visit};
use syn::Signature;
use tokei::{Config, LanguageType, Languages};

use crate::error::{Error, Result};

#[derive(Debug)]
pub struct Stat {
    pub blank_line: usize,
    pub code_line: usize,
    pub comment_line: usize,
    pub total_line: usize,
    pub num_fn: usize,
    pub num_unsafe_fn: usize,
    pub num_contains_unsafe_fn: usize,
    pub num_loop_in_unsafe_fn: usize,
    pub num_unsafe_global: usize,
    pub inaccurate: bool,
}

impl Stat {
    pub fn new() -> Self {
        Stat {
            blank_line: 0,
            code_line: 0,
            comment_line: 0,
            total_line: 0,
            num_fn: 0,
            num_unsafe_fn: 0,
            num_contains_unsafe_fn: 0,
            num_loop_in_unsafe_fn: 0,
            num_unsafe_global: 0,
            inaccurate: false,
        }
    }
}

impl std::ops::Add<&Stat> for Stat {
    type Output = Stat;

    fn add(mut self, other: &Stat) -> Stat {
        self.blank_line += other.blank_line;
        self.code_line += other.code_line;
        self.comment_line += other.comment_line;
        self.total_line += other.total_line;
        self.num_fn += other.num_fn;
        self.num_unsafe_fn += other.num_unsafe_fn;
        self.num_contains_unsafe_fn += other.num_contains_unsafe_fn;
        self.num_loop_in_unsafe_fn += other.num_loop_in_unsafe_fn;
        self.num_unsafe_global += other.num_unsafe_global;
        self.inaccurate |= other.inaccurate;
        self
    }
}

impl std::ops::AddAssign<StatVisitor<'_>> for Stat {
    fn add_assign(&mut self, visitor: StatVisitor) {
        self.num_fn += visitor.num_fn;
        self.num_unsafe_fn += visitor.num_unsafe_fn;
        self.num_contains_unsafe_fn += visitor.num_contains_unsafe_fn;
        self.num_loop_in_unsafe_fn += visitor.num_loop_in_unsafe_fn;
        self.num_unsafe_global += visitor.unsafe_global;
    }
}

impl From<tokei::Stats> for Stat {
    fn from(tokei_stat: tokei::Stats) -> Self {
        let mut stat = Stat::new();
        stat.blank_line = tokei_stat.blanks;
        stat.code_line = tokei_stat.code;
        stat.comment_line = tokei_stat.comments;
        stat.total_line = tokei_stat.lines;
        stat
    }
}

#[derive(Debug)]
pub struct CrateStat {
    pub summary: Stat,
    pub stats: Vec<(Stat, PathBuf)>,
}

struct FunctionStat {
    is_unsafe: bool,
    contains_unsafe: bool,
    loop_in_unsafe: bool,
    nested_unsafe_block: usize,
}

pub struct StatVisitor<'ast> {
    pub num_fn: usize,
    pub num_unsafe_fn: usize,
    pub num_contains_unsafe_fn: usize,
    pub num_loop_in_unsafe_fn: usize,
    pub unsafe_global: usize,
    _content: &'ast str,
    visit_stack: Vec<FunctionStat>,
}

impl<'ast> StatVisitor<'ast> {
    fn enter_fn(&mut self, sig: &Signature) {
        let is_unsafe = sig.unsafety.is_some();
        self.visit_stack.push(FunctionStat {
            is_unsafe,
            contains_unsafe: false,
            loop_in_unsafe: false,
            nested_unsafe_block: if is_unsafe { 1 } else { 0 },
        });
    }

    fn leave_fn(&mut self) {
        let item = self.visit_stack.pop().expect("bug in visitor logic");

        self.num_fn += 1;

        if item.is_unsafe {
            self.num_unsafe_fn += 1;
        } else if item.contains_unsafe {
            self.num_contains_unsafe_fn += 1;
        }

        if item.loop_in_unsafe {
            self.num_loop_in_unsafe_fn += 1;
        }
    }

    fn enter_loop(&mut self) {
        if let Some(fn_stat) = self.visit_stack.last_mut() {
            if fn_stat.nested_unsafe_block > 0 {
                fn_stat.loop_in_unsafe = true;
            }
        }
    }

    fn leave_loop(&mut self) {}

    fn enter_unsafe(&mut self) {
        match self.visit_stack.last_mut() {
            Some(fn_stat) => {
                fn_stat.contains_unsafe = true;
                fn_stat.nested_unsafe_block += 1;
            }
            None => self.unsafe_global += 1,
        }
    }

    fn leave_unsafe(&mut self) {
        if let Some(fn_stat) = self.visit_stack.last_mut() {
            fn_stat.nested_unsafe_block -= 1;
        }
    }
}

impl<'ast> StatVisitor<'ast> {
    pub fn new(content: &'ast str) -> Self {
        StatVisitor {
            num_fn: 0,
            num_unsafe_fn: 0,
            num_contains_unsafe_fn: 0,
            num_loop_in_unsafe_fn: 0,
            unsafe_global: 0,
            _content: content,
            visit_stack: Vec::new(),
        }
    }
}

impl<'ast> Visit<'ast> for StatVisitor<'ast> {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.enter_fn(&node.sig);
        visit::visit_item_fn(self, node);
        self.leave_fn();
    }

    fn visit_trait_item_method(&mut self, node: &'ast syn::TraitItemMethod) {
        self.enter_fn(&node.sig);
        visit::visit_trait_item_method(self, node);
        self.leave_fn();
    }

    fn visit_impl_item_method(&mut self, node: &'ast syn::ImplItemMethod) {
        self.enter_fn(&node.sig);
        visit::visit_impl_item_method(self, node);
        self.leave_fn();
    }

    fn visit_expr_unsafe(&mut self, node: &'ast syn::ExprUnsafe) {
        self.enter_unsafe();
        visit::visit_expr_unsafe(self, node);
        self.leave_unsafe();
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.enter_loop();
        visit::visit_expr_for_loop(self, node);
        self.leave_loop();
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.enter_loop();
        visit::visit_expr_loop(self, node);
        self.leave_loop();
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.enter_loop();
        visit::visit_expr_while(self, node);
        self.leave_loop();
    }
}

fn stat_tokei(path: &Path) -> tokei::Language {
    let config = Config {
        types: Some(vec![LanguageType::Rust]),
        ..Config::default()
    };

    let mut languages = Languages::new();
    languages.get_statistics(&[path], &["test", "tests"], &config);

    if languages.contains_key(&LanguageType::Rust) {
        languages[&LanguageType::Rust].clone()
    } else {
        tokei::Language::new()
    }
}

fn stat_syn(tokei_stat: tokei::Stats) -> (Stat, PathBuf) {
    let filename = tokei_stat.name.clone();
    let mut stat = Stat::from(tokei_stat);

    let result: Result<_> = (|| {
        let mut file = fs::File::open(&filename)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;

        if let Ok(ast) = syn::parse_file(&content) {
            let mut visitor = StatVisitor::new(&content);
            visitor.visit_file(&ast);
            stat += visitor;
        }

        Ok(())
    })();

    if result.is_err() {
        stat.inaccurate = true;
    }

    (stat, filename)
}

pub fn stat(path: &Path) -> Result<CrateStat> {
    if fs::read_dir(path)?.count() == 0 {
        return Err(Error::EmptyCrateError);
    }

    let tokei_stat = stat_tokei(path);
    if tokei_stat.stats.is_empty() {
        return Err(Error::NoRustFileError);
    }

    let stats: Vec<_> = tokei_stat.stats.into_iter().map(stat_syn).collect();
    let mut summary: Stat = stats.iter().fold(Stat::new(), |acc, x| acc + &x.0);
    summary.inaccurate |= tokei_stat.inaccurate;

    Ok(CrateStat { summary, stats })
}
