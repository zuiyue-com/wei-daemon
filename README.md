# Wei Daemon

A powerful process daemon written in Rust, featuring process monitoring, automatic restart, and health checking capabilities.

## Features

- **Process Management**
  - Manage multiple processes via configuration file
  - Automatic process status monitoring
  - Automatic restart on process failure
  - Flexible restart strategy configuration

- **Thread Management**
  - Thread health monitoring
  - Exception handling and auto-recovery
  - Configurable restart policies
  - Real-time thread status reporting

- **Exception Handling**
  - Global exception capture and handling
  - Thread crash protection
  - Detailed error logging

- **System Monitoring**
  - Periodic status reporting
  - Process and thread health checks
  - Resource usage monitoring
  - Graceful shutdown handling

## Configuration

The program uses a `daemon.dat` file to configure the list of processes to monitor. Configuration file format:

```plaintext
C:\path\to\process1.exe
C:\path\to\process2.exe
```

One process full path per line.

## System Architecture

The system consists of the following main modules:

- **ProcessManager**: Handles process lifecycle management
  - Process start, stop, and restart
  - Process status monitoring
  - Automatic recovery mechanisms

- **ThreadManager**: Manages thread operations
  - Thread creation and lifecycle management
  - Health checking
  - Restart policy implementation

- **ExceptionHandler**: Manages exception handling
  - Global exception capturing
  - Thread recovery strategies
  - Error logging

- **SignalHandler**: Handles system signals
  - System signal processing
  - Graceful shutdown support

## Usage

### Installation

```bash
cargo build --release
```

### Running

```bash
./wei-daemon
```

### Process Configuration

1. Create or edit the `daemon.dat` file
2. Add one process path per line
3. Run the daemon after saving the configuration

### Monitoring Output

The program periodically outputs status reports, including:
- Thread status
- Process status
- Restart statistics
- Exception counts

## Technical Features

- Rust concurrency and thread safety features
- Tokio-based async runtime
- Thread safety with Arc and Mutex
- Configurable retry mechanisms
- Real-time monitoring and reporting system

## Error Handling

- Detailed error logging
- Automatic process recovery
- Configurable retry strategies
- Graceful error handling procedures

## Contributing

Pull Requests and Issues are welcome. Please ensure:
- Code follows Rust formatting guidelines
- Appropriate tests are added
- Documentation is updated

## License

MIT License or Apache License 2.0