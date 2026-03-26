use std::collections::HashMap;
use strsim::{jaro_winkler, levenshtein};

pub struct LevenshteinMatcher {
    dict: Vec<String>,
    normalized_cache: HashMap<String, String>,
}

impl LevenshteinMatcher {
    pub fn new(dict: Vec<String>) -> Self {
        Self {
            dict,
            normalized_cache: HashMap::new(),
        }
    }

    pub fn match_str(&mut self, input: &str) -> Option<String> {
        if let Some(candidate) = self.normalized_cache.get(input) {
            return Some(candidate.clone());
        }

        let max_levenshtein = 1; // 编辑距离阈值
        let min_jaro = 0.80; // jaro-winkler 最低相似度
        let alpha = 0.7; // 权重：levenshtein
        let beta = 0.3; // 权重：jaro_winkler

        let candidate = self
            .dict
            .iter()
            .filter_map(|candidate| {
                let lev = levenshtein(candidate, input);
                let jw = jaro_winkler(candidate, input);

                // 编辑距离过大 或 相似度过低 → 过滤掉
                if lev > max_levenshtein && jw < min_jaro {
                    return None;
                }

                // 综合分数（越小越好）
                let score = alpha * (lev as f64) + beta * (1.0 - jw);
                Some((candidate.as_str(), score))
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(best, _)| best.to_string());

        // save to cache
        if let Some(candidate) = &candidate {
            self.normalized_cache
                .insert(input.to_string(), candidate.clone());
        }

        candidate
    }
}
