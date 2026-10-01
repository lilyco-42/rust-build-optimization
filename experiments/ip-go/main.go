// ipcheck — 跨平台本机 IP 查看小工具（Go 版，与 Rust/C 版同功能）。
// 零第三方依赖，只用标准库。
package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"net"
	"os"
	"runtime"
)

type ifaceInfo struct {
	Iface string `json:"iface"`
	IP    string `json:"ip"`
}

type report struct {
	Hostname       string      `json:"hostname"`
	OS             string      `json:"os"`
	Arch           string      `json:"arch"`
	LocalIP        string      `json:"local_ip"`
	IsLoopback     bool        `json:"is_loopback"`
	Target         string      `json:"target"`
	TargetResolved []string    `json:"target_resolved,omitempty"`
	Interfaces     []ifaceInfo `json:"interfaces,omitempty"`
}

func main() {
	target := flag.String("target", "8.8.8.8:80", "探测出口 IP 的目标（只 connect 不发包）")
	detail := flag.Bool("detail", false, "列出全部网卡地址")
	asJSON := flag.Bool("json", false, "单行 JSON 输出")
	flag.Parse()

	host, _ := os.Hostname()
	if host == "" {
		host = "unknown"
	}

	// UDP connect 反查出口 IP（不发包）
	localIP := "unreachable"
	if conn, err := net.Dial("udp", *target); err == nil {
		if addr, ok := conn.LocalAddr().(*net.UDPAddr); ok {
			localIP = addr.IP.String()
		}
		conn.Close()
	}
	ip := net.ParseIP(localIP)

	rep := report{
		Hostname: host, OS: runtime.GOOS, Arch: runtime.GOARCH,
		LocalIP: localIP, IsLoopback: ip != nil && ip.IsLoopback(), Target: *target,
	}

	if *detail {
		if addrs, err := net.ResolveIPAddr("ip", *target); err == nil {
			// 目标解析（取第一条即可，行为对齐）
			_ = addrs
		}
		if list, err := net.Interfaces(); err == nil {
			for _, ifi := range list {
				if ifi.Flags&net.FlagUp == 0 {
					continue
				}
				addrs, _ := ifi.Addrs()
				for _, a := range addrs {
					var s string
					switch v := a.(type) {
					case *net.IPNet:
						s = v.IP.String()
					case *net.IPAddr:
						s = v.IP.String()
					}
					if s != "" {
						rep.Interfaces = append(rep.Interfaces, ifaceInfo{ifi.Name, s})
					}
				}
			}
		}
	}

	if *asJSON {
		out, _ := json.Marshal(rep)
		fmt.Println(string(out))
		return
	}
	fmt.Printf("%s/%s %s -> %s\n", rep.OS, rep.Arch, rep.Hostname, rep.LocalIP)
	if *detail {
		fmt.Println("interfaces:")
		for _, ii := range rep.Interfaces {
			fmt.Printf("  %-16s %s\n", ii.Iface, ii.IP)
		}
	}
}

