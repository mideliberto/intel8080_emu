# Board artifacts. GAL: galette 0.3.0 (cargo install galette --version 0.3.0 --locked).
# Commit glue.pld and glue.jed together; tests/gal_tests.rs reads both and needs no galette.

GALETTE = galette

.PHONY: all

all: glue.jed

# -c -f -p: no .chp, .fus or .pin; the .jed is the only output kept.
glue.jed: glue.pld
	$(GALETTE) -c -f -p glue.pld
