 架构设计建议

  1. 数据结构扩展

  // 进程配置信息
  pub struct ProcessConfig {
      pub name: String,           // 进程名称
      pub executable_path: String, // 可执行文件路径
      pub working_directory: String, // 工作目录
      pub arguments: Vec<String>, // 启动参数
      pub restart_policy: ProcessRestartPolicy, // 重启策略
  }

  // 进程状态
  pub enum ProcessStatus {
      Stopped,     // 已停止
      Starting,    // 启动中
      Running,     // 运行中
      Stopping,    // 停止中
      Failed,      // 失败
      Restarting,  // 重启中
  }

  // 进程信息
  pub struct ProcessInfo {
      pub config: ProcessConfig,
      pub status: ProcessStatus,
      pub process_handle: Option<Child>, // 子进程句柄
      pub start_time: SystemTime,
      pub restart_count: u32,
      pub last_heartbeat: SystemTime,
  }

  具体编写提示词

  提示词1: 配置文件解析器

  请帮我实现一个配置文件解析器：

  1. 创建 `config_parser.rs` 模块
  2. 实现读取 `daemon.dat` 文件的功能：
     - 每行一个应用程序名称
     - 支持 # 开头的注释行
     - 忽略空行和空白字符
  3. 根据应用程序名称推断可执行文件路径：
     - 假设所有应用都在当前目录的同级目录
     - 例如：如果当前在 `/path/wei-daemon`，应用 `app1` 在
  `/path/app1/app1.exe`
  4. 提供配置重新加载功能，支持运行时更新
  5. 添加配置验证：检查可执行文件是否存在
  6. 支持配置文件变更监控（可选）

  请使用现有的日志系统 `log_info`, `log_warn`, `log_error`。

  提示词2: 进程管理器

  请帮我实现一个进程管理器 `ProcessManager`：

  1. 创建 `process_manager.rs` 模块
  2. 实现以下核心功能：
     - `start_process(name: &str)` - 启动指定进程
     - `stop_process(name: &str)` - 停止指定进程
     - `restart_process(name: &str)` - 重启指定进程
     - `get_process_status(name: &str)` - 获取进程状态
     - `list_all_processes()` - 列出所有进程状态
  3. 集成现有的异常处理和重启机制
  4. 使用 `std::process::Command` 启动子进程
  5. 实现进程监控：
     - 定期检查进程是否存在（通过process handle）
     - 进程异常退出时自动重启
     - 支持优雅停止和强制终止
  6. 状态持久化：记录进程启动时间、重启次数等
  7. 与现有的线程管理系统集成，为每个进程分配监控线程

  请确保与现有的 ThreadManager 和异常处理系统兼容。

  提示词3: 进程监控线程

  请帮我实现进程监控功能：

  1. 为每个管理的进程创建一个监控线程
  2. 监控线程职责：
     - 定期检查进程健康状态（每5秒检查一次）
     - 检测进程是否意外终止
     - 根据重启策略决定是否重启
     - 收集进程资源使用情况（CPU、内存）- 可选
  3. 使用现有的线程管理系统创建监控线程
  4. 集成异常处理：监控线程本身也要有异常保护
  5. 实现进程通信检查：
     - 可以通过文件、命名管道或网络检查进程响应
     - 超时未响应视为进程异常
  6. 支持不同的监控策略：
     - 简单存活检查
     - 健康状态检查
     - 性能监控

  使用现有的 `safe_thread_wrapper` 和重启机制。

  提示词4: 主程序集成

  请帮我修改 main.rs 文件，集成进程管理功能：

  1. 在程序启动时：
     - 加载 `daemon.dat` 配置文件
     - 初始化 ProcessManager
     - 启动所有配置的进程
  2. 替换或扩展现有的 Worker 线程创建逻辑
  3. 在主循环中：
     - 显示所有进程的状态（而不是线程状态）
     - 监控配置文件变化，支持热重载
     - 处理进程管理命令（如果需要）
  4. 在程序退出时：
     - 优雅停止所有管理的进程
     - 等待进程正常退出或强制终止
  5. 保持现有的信号处理和异常处理功能
  6. 状态报告应该显示：
     - 进程名称、状态、PID
     - 运行时间、重启次数
     - 资源使用情况（可选）

  确保与现有的 SignalHandler 和 ExceptionHandler 兼容。

  提示词5: 配置和日志增强

  请帮我增强配置和日志功能：

  1. 扩展配置支持：
     - 为每个进程支持自定义启动参数
     - 支持环境变量设置
     - 支持工作目录配置
     - 支持重启策略配置（每个进程独立配置）
  2. 改进 daemon.dat 格式，支持更多配置选项：
  进程名称:可执行文件:工作目录:启动参数:最大重启次数

     app1:./app1.exe:.:--config=app1.conf:5
     app2:./app2.exe:../app2:--port=8080:3
  3. 增强日志输出：
  - 为每个进程添加独立的日志文件
  - 支持日志轮转和归档
  - 进程启动/停止/重启事件记录
  - 异常和错误详细记录
  4. 支持配置验证和错误处理
  5. 提供配置重载命令或信号处理

  请保持向后兼容，支持简单的每行一个进程名的格式。

  提示词6: 进程间通信和控制

  请帮我实现进程控制接口：

  1. 创建控制接口模块 `control_interface.rs`：
     - 支持命令行参数控制（start, stop, restart, status）
     - 支持配置文件重载命令
     - 支持查询所有进程状态
  2. 实现简单的IPC机制：
     - 可以通过命名管道、文件锁或网络接口
     - 支持外部工具查询和控制守护进程
  3. 添加管理命令：
     ```bash
     wei-daemon --start app1        # 启动指定进程
     wei-daemon --stop app1         # 停止指定进程
     wei-daemon --restart app1      # 重启指定进程
     wei-daemon --status            # 查看所有进程状态
     wei-daemon --reload            # 重载配置文件
  4. 实现进程组管理：
    - 支持批量启动/停止
    - 支持依赖关系管理（可选）
  5. 添加Web接口或REST API（可选高级功能）

  确保控制接口也有适当的异常处理和日志记录。

  ## 实施顺序建议

  1. **首先实现配置解析器** - 解析daemon.dat文件
  2. **然后实现进程管理器** - 核心的进程启动/停止逻辑
  3. **集成监控线程** - 利用现有线程管理系统
  4. **修改主程序** - 替换示例Worker线程
  5. **增强配置和日志** - 提升功能完整性
  6. **添加控制接口** - 提供外部控制能力



 1. 主进程框架
  帮我创建一个Windows多线程守护进程的主框架，包含：
  - 主函数入口点
  - 全局退出标志变量
  - 基础的日志输出函数
  - 程序启动和初始化流程

  2. 线程管理模块
  实现线程管理功能：
  - 创建ThreadManager类管理工作线程
  - 实现创建、启动、停止线程的方法
  - 添加线程状态监控和健康检查
  - 使用线程安全的容器存储线程句柄

  3. 信号处理
  添加Windows控制台信号处理：
  - 实现SetConsoleCtrlHandler回调函数
  - 处理CTRL_C_EVENT和CTRL_CLOSE_EVENT
  - 设置全局退出标志，通知所有线程优雅退出
  - 添加超时强制退出机制

  4. 异常处理系统
  实现全面的异常处理机制：
  - 为每个工作线程添加try-catch包装
  - 实现SEH结构化异常处理
  - 创建异常信息收集和格式化函数
  - 添加异常发生时的线程重启逻辑

  5. 日志记录系统
  创建线程安全的日志系统：
  - 实现文件日志写入功能
  - 添加日志级别（DEBUG, INFO, WARN, ERROR）
  - 支持日志文件轮转和大小限制
  - 确保多线程并发写入安全

  6. 工作线程示例
  创建示例工作线程：
  - 实现一个或多个具体的工作线程功能
  - 展示线程间通信方式
  - 添加线程工作状态报告机制
  - 实现可配置的工作间隔和参数