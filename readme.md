# About
This program checks whether and how one-dimensional heat transport can be calculated on a graphics card. For this purpose, a CPU algorithm and 3 GPU algorithms were created. The heat transport itself was implemented according to the [FDS Technical Reference Guide](https://pages.nist.gov/fds-smv/downloads.html). This programme was created as part of the master thesis "Ausführung eines Wärmetransportalgorithmus auf einer GPU". The program was validated against [FDS 6.8.0](https://github.com/firemodels/fds).

# Preparation 
Before the program can be started, it must first be checked that the links are present.
At path `fds/1D/Diabatic/multiple` the result directory in `fds/1D/Diabatic/multiple/16` must be linked in `1`, `2`, `4` and `8`

# Start the program
The program can be run with or without the plots being created. The following command can be used to display all settings for starting the program.

```
cargo run -- -h` 
```

## Start without plots
```
cargo run --release -- -s -b <NAME>
```
## Plot the results
In order to plot the results, the conditions for the module [`plotly`](https://github.com/igiagkiozis/plotly) with the feature `kaleido` must be fulfilled.

```
cargo run --features plot -- -p all
```
## Start on Linux
- Install FDS
- run `cargo run --release -- -b <NAME>`

## Start on Windows
- Install FDS
- Open the created FDS CMD shortcut on the desktop
- Navigate to this project folder
- run `cargo run --release -- -b <NAME>`