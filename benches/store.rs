use std::{path::Path, error::Error, io::Write};

use criterion::{criterion_group, criterion_main, Criterion};
use rand::{seq::IteratorRandom, Rng, SeedableRng};
use lumin::store::{EXTENSIONS, find_and_process};


struct TreeGenerator {
    rng: rand::rngs::StdRng,
    max_depth: usize,
    dirs: usize,
    matching_files: usize,
    extra_files: usize,
    file_sizes: &'static [usize],
}

impl TreeGenerator {
    fn generate_filename(&mut self) -> String {
        (&mut self.rng)
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(8)
            .map(char::from)
            .collect()
    }

    fn random_buffer(&mut self) -> Vec<u8> {
        let sz = *self.file_sizes.iter().choose(&mut self.rng).unwrap_or(&1);
        std::iter::repeat(0).take(sz).collect()
    }

    fn generate_inner(&mut self, path: &Path, current_depth: usize) -> Result<(), Box<dyn Error>> {
        if current_depth >= self.max_depth {
            return Ok(());
        }

        for i in 0..self.matching_files {
            let mut path = path.join(self.generate_filename());
            path.set_extension(EXTENSIONS[i as usize % EXTENSIONS.len()]);

            let mut f = std::fs::File::create(path)?;
            let buf = self.random_buffer();
            f.write_all(&buf)?;
        }

        for _ in 0..self.extra_files {
            let buf = self.random_buffer();
            let mut f = std::fs::File::create(path.join(self.generate_filename()))?;
            f.write_all(&buf)?;
        }

        for _ in 0..self.dirs {
            let path = path.join(self.generate_filename());
            std::fs::create_dir(&path)?;

            self.generate_inner(&path, current_depth + 1)?;
        }

        Ok(())
    }

    fn generate(&mut self, path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
        self.generate_inner(path.as_ref(), 0)
    }
}

fn bench_find_and_process(c: &mut Criterion) {
    let rng = rand::rngs::StdRng::seed_from_u64(0);
    let mut generators: [TreeGenerator; 1] = [TreeGenerator {
        rng: rng.clone(),
        dirs: 3,
        matching_files: 20,
        extra_files: 5,
        max_depth: 3,
        file_sizes: &[1024, 16 * 1024, 1024 * 1024],
    }];

    for gen in &mut generators {
        c.bench_function(
            &format!(
                "dirs={},matching_files={},extra_files={},max_depth={},file_sizes={:?}",
                gen.dirs, gen.matching_files, gen.extra_files, gen.max_depth, gen.file_sizes
            ),
            |b| {
                let tmp = std::env::temp_dir().join(gen.generate_filename());
                std::fs::create_dir(&tmp).unwrap();

                gen.generate(&tmp).unwrap();

                b.iter(|| find_and_process(&tmp));

                std::fs::remove_dir_all(&tmp).unwrap();
            },
        );
    }
}

criterion_group!(benches, bench_find_and_process);
criterion_main!(benches);
