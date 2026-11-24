use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::ffi::CStr;
use std::mem::{size_of, zeroed};
use std::ptr::null_mut;
use std::time::Instant;

const MAX_HISTORY: usize = 600;
const PROC_PIDFDATSOCKET: i32 = 3;
const SOCKINFO_IN: i32 = 1;
const SOCKINFO_TCP: i32 = 2;
const TSI_S_LISTEN: i32 = 1;

#[repr(C)]
struct Timebase {
    numer: u32,
    denom: u32,
}

unsafe extern "C" {
    fn mach_timebase_info(info: *mut Timebase) -> i32;
    fn mach_host_self() -> libc::mach_port_t;
}

#[repr(C)]
#[derive(Copy, Clone)]
struct ProcFileInfo {
    fi_openflags: u32,
    fi_status: u32,
    fi_offset: i64,
    fi_type: i32,
    fi_guardflags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct VinfoStat {
    vst_dev: u32,
    vst_mode: u16,
    vst_nlink: u16,
    vst_ino: u64,
    vst_uid: u32,
    vst_gid: u32,
    vst_atime: i64,
    vst_atimensec: i64,
    vst_mtime: i64,
    vst_mtimensec: i64,
    vst_ctime: i64,
    vst_ctimensec: i64,
    vst_birthtime: i64,
    vst_birthtimensec: i64,
    vst_size: i64,
    vst_blocks: i64,
    vst_blksize: i32,
    vst_flags: u32,
    vst_gen: u32,
    vst_rdev: u32,
    vst_qspare: [i64; 2],
}

#[repr(C)]
#[derive(Copy, Clone)]
struct SockbufInfo {
    sbi_cc: u32,
    sbi_hiwat: u32,
    sbi_mbcnt: u32,
    sbi_mbmax: u32,
    sbi_lowat: u32,
    sbi_flags: i16,
    sbi_timeo: i16,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct In4Info {
    in4_tos: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct In6Info {
    in6_hlim: u8,
    in6_cksum: i32,
    in6_ifindex: u16,
    in6_hops: i16,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct InSockInfo {
    insi_fport: i32,
    insi_lport: i32,
    insi_gencnt: u64,
    insi_flags: u32,
    insi_flow: u32,
    insi_vflag: u8,
    insi_ip_ttl: u8,
    rfu_1: u32,
    insi_faddr: [u8; 16],
    insi_laddr: [u8; 16],
    insi_v4: In4Info,
    insi_v6: In6Info,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct TcpSockInfo {
    tcpsi_ini: InSockInfo,
    tcpsi_state: i32,
    tcpsi_timer: [i32; 4],
    tcpsi_mss: i32,
    tcpsi_flags: u32,
    rfu_1: u32,
    tcpsi_tp: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct UnSockInfo {
    unsi_conn_so: u64,
    unsi_conn_pcb: u64,
    unsi_addr: [u8; 255],
    unsi_caddr: [u8; 255],
}

#[repr(C)]
#[derive(Copy, Clone)]
union SocketProto {
    pri_in: InSockInfo,
    pri_tcp: TcpSockInfo,
    pri_un: UnSockInfo,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct SocketInfo {
    soi_stat: VinfoStat,
    soi_so: u64,
    soi_pcb: u64,
    soi_type: i32,
    soi_protocol: i32,
    soi_family: i32,
    soi_options: i16,
    soi_linger: i16,
    soi_state: i16,
    soi_qlen: i16,
    soi_incqlen: i16,
    soi_qlimit: i16,
    soi_timeo: i16,
    soi_error: u16,
    soi_oobmark: u32,
    soi_rcv: SockbufInfo,
    soi_snd: SockbufInfo,
    soi_kind: i32,
    rfu_1: u32,
    soi_proto: SocketProto,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct SocketFdInfo {
    pfi: ProcFileInfo,
    psi: SocketInfo,
}

#[derive(Clone, Serialize)]
pub struct Proc {
    pub pid: i32,
    pub ppid: i32,
    pub name: String,
    pub cpu: f64,
    pub mem: u64,
    pub delta: i64,
    pub ports: Vec<u16>,
    pub children: Vec<Proc>,
}

#[derive(Clone, Serialize)]
pub struct Tick {
    pub ts: u64,
    pub total_mem: u64,
    pub pressure: u8,
    pub pressure_level: i32,
    pub procs: Vec<Proc>,
}

pub struct Sampler {
    prev_cpu: HashMap<i32, u64>,
    prev_mem: HashMap<i32, u64>,
    last_at: Instant,
    numer: u64,
    denom: u64,
    uid: u32,
    pub history: VecDeque<Tick>,
}

impl Sampler {
    pub fn new() -> Self {
        let mut tb = Timebase { numer: 1, denom: 1 };
        unsafe { mach_timebase_info(&mut tb) };
        Self {
            prev_cpu: HashMap::new(),
            prev_mem: HashMap::new(),
            last_at: Instant::now(),
            numer: tb.numer as u64,
            denom: tb.denom as u64,
            uid: unsafe { libc::getuid() },
            history: VecDeque::with_capacity(MAX_HISTORY),
        }
    }

    pub fn sample(&mut self, epoch: u64) -> Tick {
        let dt = self.last_at.elapsed().as_nanos().max(1) as f64;
        self.last_at = Instant::now();

        let mut flat = HashMap::new();
        let mut next_cpu = HashMap::new();
        let mut next_mem = HashMap::new();
        let (total_mem, pressure, pressure_level) = read_sys_mem();

        for pid in list_pids() {
            let Some((ppid, uid, name)) = read_bsd(pid) else { continue };
            let ticks = read_ticks(pid);
            let mem = read_mem(pid);
            let ports = if uid == self.uid { read_ports(pid) } else { Vec::new() };

            let cpu = match self.prev_cpu.get(&pid) {
                Some(&prev) if ticks >= prev => {
                    let ns = ((ticks - prev) * self.numer / self.denom) as f64;
                    (ns / dt) * 100.0
                }
                _ => 0.0,
            };

            let delta = match self.prev_mem.get(&pid) {
                Some(&prev) => mem as i64 - prev as i64,
                None => 0,
            };

            next_cpu.insert(pid, ticks);
            next_mem.insert(pid, mem);

            flat.insert(pid, Proc {
                pid,
                ppid,
                name,
                cpu,
                mem,
                delta,
                ports,
                children: Vec::new(),
            });
        }

        self.prev_cpu = next_cpu;
        self.prev_mem = next_mem;

        let procs = build_tree(flat);
        let tick = Tick {
            ts: epoch,
            total_mem,
            pressure,
            pressure_level,
            procs,
        };

        if self.history.len() == MAX_HISTORY {
            self.history.pop_front();
        }
        self.history.push_back(tick.clone());
        tick
    }
}

fn read_sysctl_i32(name: &[u8]) -> Option<i32> {
    let mut val: i32 = 0;
    let mut len = size_of::<i32>();
    let ret = unsafe {
        libc::sysctlbyname(
            name.as_ptr() as *const _,
            &mut val as *mut _ as *mut _,
            &mut len,
            null_mut(),
            0,
        )
    };
    if ret == 0 { Some(val) } else { None }
}

fn read_sys_mem() -> (u64, u8, i32) {
    let mut vm: libc::vm_statistics64 = unsafe { zeroed() };
    let mut count = libc::HOST_VM_INFO64_COUNT;
    let ret = unsafe {
        libc::host_statistics64(
            mach_host_self(),
            libc::HOST_VM_INFO64,
            &mut vm as *mut _ as *mut libc::integer_t,
            &mut count,
        )
    };
    let used = if ret == libc::KERN_SUCCESS {
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
        let app = vm.internal_page_count.saturating_sub(vm.purgeable_count) as u64;
        let wired = vm.wire_count as u64;
        let compressed = vm.compressor_page_count as u64;
        (app + wired + compressed) * page
    } else {
        0
    };

    let level = read_sysctl_i32(b"kern.memorystatus_vm_pressure_level\0").unwrap_or(1);
    let pressure = match read_sysctl_i32(b"kern.memorystatus_level\0") {
        Some(avail) => (100 - avail).clamp(0, 100) as u8,
        None => {
            let mut total: u64 = 0;
            let mut len = size_of::<u64>();
            let ok = unsafe {
                libc::sysctlbyname(
                    b"hw.memsize\0".as_ptr() as *const _,
                    &mut total as *mut _ as *mut _,
                    &mut len,
                    null_mut(),
                    0,
                )
            };
            if ok == 0 && total > 0 {
                ((used.saturating_mul(100)) / total).min(100) as u8
            } else {
                0
            }
        }
    };

    (used, pressure, level)
}

fn list_pids() -> Vec<i32> {
    let n = unsafe { libc::proc_listallpids(null_mut(), 0) };
    if n <= 0 {
        return Vec::new();
    }
    let mut pids = vec![0i32; n as usize + 64];
    let actual = unsafe {
        libc::proc_listallpids(
            pids.as_mut_ptr() as *mut _,
            (pids.len() * size_of::<i32>()) as i32,
        )
    };
    if actual <= 0 {
        return Vec::new();
    }
    pids.truncate(actual as usize);
    pids.retain(|&p| p > 0);
    pids
}

fn read_full_name(pid: i32) -> Option<String> {
    let mut buf = [0u8; 4096];
    let ret = unsafe {
        libc::proc_pidpath(pid, buf.as_mut_ptr() as *mut _, buf.len() as u32)
    };
    if ret <= 0 {
        return None;
    }
    let path = String::from_utf8_lossy(&buf[..ret as usize]);
    path.rsplit('/').next().filter(|s| !s.is_empty()).map(String::from)
}

fn read_bsd(pid: i32) -> Option<(i32, u32, String)> {
    let mut info: libc::proc_bsdinfo = unsafe { zeroed() };
    let sz = size_of::<libc::proc_bsdinfo>() as i32;
    let ret = unsafe {
        libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, &mut info as *mut _ as *mut _, sz)
    };
    if ret != sz {
        return None;
    }
    let raw = if info.pbi_name[0] != 0 {
        info.pbi_name.as_ptr()
    } else {
        info.pbi_comm.as_ptr()
    };
    let mut name = unsafe { CStr::from_ptr(raw) }.to_string_lossy().into_owned();
    if name.len() >= 30 {
        if let Some(full) = read_full_name(pid) {
            name = full;
        }
    }
    Some((info.pbi_ppid as i32, info.pbi_uid, name))
}

fn read_ticks(pid: i32) -> u64 {
    let mut info: libc::proc_taskinfo = unsafe { zeroed() };
    let sz = size_of::<libc::proc_taskinfo>() as i32;
    let ret = unsafe {
        libc::proc_pidinfo(pid, libc::PROC_PIDTASKINFO, 0, &mut info as *mut _ as *mut _, sz)
    };
    if ret != sz {
        return 0;
    }
    info.pti_total_user + info.pti_total_system
}

fn read_mem(pid: i32) -> u64 {
    let mut ru: libc::rusage_info_v4 = unsafe { zeroed() };
    let ret = unsafe {
        libc::proc_pid_rusage(pid, libc::RUSAGE_INFO_V4, &mut ru as *mut _ as *mut _)
    };
    if ret == 0 { ru.ri_phys_footprint } else { 0 }
}

fn read_ports(pid: i32) -> Vec<u16> {
    let fd_sz = size_of::<libc::proc_fdinfo>() as i32;
    let bytes = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, null_mut(), 0) };
    if bytes <= 0 {
        return Vec::new();
    }
    let mut fds = vec![unsafe { zeroed::<libc::proc_fdinfo>() }; (bytes / fd_sz) as usize + 16];
    let actual = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDLISTFDS,
            0,
            fds.as_mut_ptr() as *mut _,
            (fds.len() as i32) * fd_sz,
        )
    };
    if actual <= 0 {
        return Vec::new();
    }
    fds.truncate((actual / fd_sz) as usize);

    let mut ports = Vec::new();
    let si_sz = size_of::<SocketFdInfo>() as i32;

    for fd in fds {
        if fd.proc_fdtype != libc::PROX_FDTYPE_SOCKET as u32 {
            continue;
        }
        let mut si: SocketFdInfo = unsafe { zeroed() };
        let ret = unsafe {
            libc::proc_pidfdinfo(
                pid,
                fd.proc_fd,
                PROC_PIDFDATSOCKET,
                &mut si as *mut _ as *mut _,
                si_sz,
            )
        };
        if ret != si_sz {
            continue;
        }
        let port = match si.psi.soi_kind {
            SOCKINFO_TCP => unsafe {
                let tcp = si.psi.soi_proto.pri_tcp;
                if tcp.tcpsi_state == TSI_S_LISTEN {
                    u16::from_be(tcp.tcpsi_ini.insi_lport as u16)
                } else {
                    0
                }
            },
            SOCKINFO_IN => unsafe {
                u16::from_be(si.psi.soi_proto.pri_in.insi_lport as u16)
            },
            _ => 0,
        };
        if port > 0 {
            ports.push(port);
        }
    }

    ports.sort_unstable();
    ports.dedup();
    ports
}

fn build_tree(mut map: HashMap<i32, Proc>) -> Vec<Proc> {
    let mut child_map: HashMap<i32, Vec<i32>> = HashMap::new();
    let mut roots = Vec::new();

    for (&pid, p) in &map {
        if p.ppid > 1 && p.ppid != pid && map.contains_key(&p.ppid) {
            child_map.entry(p.ppid).or_default().push(pid);
        } else {
            roots.push(pid);
        }
    }

    fn attach(pid: i32, map: &mut HashMap<i32, Proc>, child_map: &HashMap<i32, Vec<i32>>) -> Proc {
        let mut node = map.remove(&pid).unwrap();
        if let Some(kids) = child_map.get(&pid) {
            for &kid in kids {
                let c = attach(kid, map, child_map);
                node.cpu += c.cpu;
                node.mem += c.mem;
                node.delta += c.delta;
                for &pt in &c.ports {
                    if !node.ports.contains(&pt) {
                        node.ports.push(pt);
                    }
                }
                node.children.push(c);
            }
            node.ports.sort_unstable();
            node.children.sort_by(|a, b| b.mem.cmp(&a.mem));
        }
        node
    }

    let mut tree: Vec<Proc> = roots
        .into_iter()
        .map(|pid| attach(pid, &mut map, &child_map))
        .collect();
    tree.sort_by(|a, b| b.mem.cmp(&a.mem));
    tree
}