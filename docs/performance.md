# Benchmark 与 profiler

这一套工具用于第 30 章优化前后的测量。benchmark 回答“同样的工作快了多少”，
profiler 回答“工作发生在哪里”。哈希表探测优化的实现与测量记录见第 4 节。

## 1. 建立基线

在仓库根目录运行：

```powershell
cargo run -p rblox-vm --bin rblox-bench --release -- --list
cargo run -p rblox-vm --bin rblox-bench --release -- --csv target/benchmarks/baseline.csv
```

默认每组执行 100000 次循环，预热 2 次，正式采样 7 次。可以只测一组：

```powershell
cargo run -p rblox-vm --bin rblox-bench --release -- --filter zoo --iterations 100000 --warmup 2 --samples 9 --csv target/benchmarks/zoo-before.csv
```

| 负载 | 主要涉及的操作 |
| --- | --- |
| arithmetic | 局部变量、数字加法、循环和比较 |
| globals | 全局变量的哈希表读取和写入 |
| zoo | 6 个字段和 6 个方法，直接调用，源自本章 Zoo 示例 |
| closures | 函数调用、关闭后的 upvalue 读取和写入 |
| strings | 有界字符串拼接与驻留、实例分配、字段写入和 GC |
| inheritance | 子类方法调用、`super` 调用、父类字段读取 |

这些是微基准，每组都校验计算结果，失败会返回非零退出码。它们帮助定位具体操作，
不能代表所有 Lox 程序；实际优化时应再加入有代表性的用户脚本。

每次采样创建一个全新 VM，避免前一轮的全局变量、对象或 GC 阈值影响下一轮。
分别用 Rust `Instant` 测量编译和执行，预热数据不写进 CSV。
执行时间包括脚本自身的类/函数初始化、执行中的 GC 和脚本结束后的 GC，
不包括文件读取、编译、VM 构造或 VM 析构。热循环不打印，不反复调用 `clock()`。

控制台显示编译时间中位数、执行时间最小值/中位数/最大值，单位都是毫秒；
CSV 保存每个正式样本。优化后用相同参数写入另一个文件：

```powershell
cargo run -p rblox-vm --bin rblox-bench --release -- --csv target/benchmarks/after.csv
```

比较相同 workload、iterations 下 `execute_ms` 的中位数：
`加速倍数 = before / after`，`耗时降低比例 = (before - after) / before`。
同时记录 Git 版本、工作区改动和 `rustc --version`，保持机器、构建配置及后台负载尽量一致。
波动范围大于改进幅度时，增加工作量并重复测量，不据单个样本下结论。

计时入口拒绝 debug 构建和 `profiling` / `debug-gc` / `stress-gc` feature，
避免把插桩或压力回收造成的变化当作真实性能变化。`--emit` 和 `--list` 不受这个限制。

## 2. 查看 Lox 函数和字节码

先导出一个可独立运行的基准脚本，再使用插桩构建：

```powershell
cargo run -p rblox-vm --bin rblox-bench --release -- --filter zoo --iterations 10000 --emit target/benchmarks/zoo.lox
cargo run -p rblox-vm --bin rblox-vm --release --features profiling -- --profile target/benchmarks/zoo.lox
```

也可以把最后的路径换成自己的 `.lox` 文件。脚本输出仍在 stdout，统计报告在 stderr；
编译/运行错误的退出码保留，运行时报错也会打印已经收集的部分统计。
需要保存报告时，先构建，再直接运行，避免把 Cargo 的构建日志混进报告：

```powershell
cargo build -p rblox-vm --bin rblox-vm --release --features profiling
.\target\release\rblox-vm.exe --profile target/benchmarks/zoo.lox 2> target/benchmarks/lox-profile.txt
```

报告包含：

- `calls`：成功进入的 Lox 调用次数，包含脚本帧、方法、初始化器及递归调用。
- `instructions`：该函数自身分派的字节码数，不把操作数或子函数指令算进去。
- `self ms`：该函数累计墙钟时间减去子 Lox 调用的时间。
- `inclusive ms`：包含子调用的累计墙钟时间。递归与父子函数之间会重叠，不能把这一列相加。
- opcode `count/share`：指令次数及其占总次数比例；**次数占比不是耗时占比**。
- GC 次数与耗时：包含执行中、脚本结束及手动触发的回收。执行中的 GC 已包含在函数耗时中，
  不要重复相加；脚本帧退出后的 GC 不计入该帧时间。

原生函数如 `clock()` 的耗时归调用者，不额外列成 Lox 函数。
函数以堆中的 prototype 标识：同一个声明生成的多个闭包合并，
不同声明即使同名也分开，用报告的 `id` 区分。元数据只保存名称和不持有对象的标识，
不会为了统计把本应回收的 Lox 对象留下来。

实现只在函数进入/退出时读取时钟，逐条指令只计数，避免给每个小指令加两次读时钟。
它仍会扰动执行时间，计数和调用关系更适合解释程序行为，真实性能比较用未插桩 benchmark。
没有启用 `profiling` feature 时，VM 中的统计字段和采集代码由条件编译移除。

库调用方可以使用 `VM::set_profiling(true)` 开始或清空统计，
`VM::profile()` 读取报告，`set_profiling(false)` 关闭。只能在执行间隙切换；
连续 `interpret()` 默认累积统计，错误退出会关闭未完成的统计栈帧。

## 3. 查看 Rust 函数的 CPU 热点（Windows）

仓库的 Cargo `profiling` **构建 profile** 继承 release 优化，并保留完整调试信息。
它与上面的 `profiling` **feature** 是两个独立概念：原生采样使用前者，不开启后者。

`scripts/profile-native.ps1` 使用 Windows Performance Recorder（WPR）采样，
由 Windows Performance Analyzer（WPA）查看。WPR 的 CPU 采样需要管理员 PowerShell。
本机已安装 WPR 和 WPA；其他机器若缺 WPA，可安装 Windows Performance Toolkit。

在管理员 PowerShell 的仓库根目录运行：

```powershell
.\scripts\profile-native.ps1 -Benchmark zoo -Iterations 3000000
```

脚本会：

1. 构建带符号的优化版 VM 与 benchmark 工具，关闭所有插桩 feature。
2. 在 `target/profiles/` 创建独立目录，保存工作负载、版本信息、EXE 和对应 PDB。
3. 使用唯一 WPR session 名启动 CPU 采样，只运行这一个工作负载。
4. 即使工作负载报错，也在 `finally` 中停止自己的 session，保存 `cpu.etl`。

不录制，只验证构建与准备文件（无需管理员权限）：

```powershell
.\scripts\profile-native.ps1 -Benchmark zoo -Iterations 10000 -PrepareOnly
```

打开输出目录中的 `cpu.etl`，在 WPA 中：

1. 加入 **CPU Usage (Sampled)**，过滤进程为 `rblox-vm.exe`。
2. 加载符号（Trace → Load Symbols）。EXE 与 PDB 已保存在同一目录；
   如仍未解析，将该目录加入符号搜索路径。
3. 展开 Stack，按采样权重查看 `VM::run`、`Table`、`Heap` 等函数及其调用者。
4. inclusive 权重包含被调用函数，self/exclusive 权重表示函数自身。
   release 下小函数可能被内联，按调用栈及源码位置分析，不能假定每个 Rust 函数都有单独一行。

WPR 会录制系统 CPU 活动，分析时需过滤目标进程。录制包含进程启动和 Lox 编译，
应让工作负载运行数秒，使执行阶段占主要部分；需要时在 WPA 中选择执行阶段时间区间。
采样比例近似 CPU 时间分布，与 Lox profiler 的插桩墙钟时间不同。

## 参考

- [Crafting Interpreters：Measuring Performance](https://craftinginterpreters.com/optimization.html#measuring-performance)
- [Crafting Interpreters：Faster Hash Table Probing](https://craftinginterpreters.com/optimization.html#faster-hash-table-probing)
- [Cargo：构建 profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)
- [Microsoft：WPR 命令行选项](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/wpr-command-line-options)
- [Microsoft：WPR/WPA 分析流程](https://learn.microsoft.com/en-us/troubleshoot/windows-server/support-tools/support-tools-xperf-wpa-wpr)
