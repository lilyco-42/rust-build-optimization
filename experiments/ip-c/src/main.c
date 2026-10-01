/* ipcheck — 跨平台本机 IP 查看小工具（xmake C 版，与 Rust/lilyco 版同功能）。
 *
 *   ip-c                    本机出口 IP + 主机名/平台
 *   ip-c --detail           列出全部网卡单播地址
 *   ip-c --json             单行 JSON 输出（手写，无第三方库）
 *   ip-c --target 1.1.1.1:80  换探测目标（只 connect 不发包）
 *
 * 零第三方依赖：Windows 用 iphlpapi + ws2_32，POSIX 用 getifaddrs，
 * 所以不需要走任何包仓库（xmake-mirror 自然零消耗）。
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0600
#endif
#include <winsock2.h>
#include <ws2tcpip.h>
#include <iphlpapi.h>
#include <windows.h>
#else
#include <unistd.h>
#include <ifaddrs.h>
#include <netdb.h>
#include <arpa/inet.h>
#include <sys/socket.h>
#include <netinet/in.h>
#endif

#if defined(_WIN32)
#define OS_NAME "windows"
#elif defined(__ANDROID__)
#define OS_NAME "android"
#elif defined(__linux__)
#define OS_NAME "linux"
#elif defined(__APPLE__)
#define OS_NAME "macos"
#else
#define OS_NAME "unknown"
#endif

#if defined(_M_X64) || defined(__x86_64__)
#define ARCH_NAME "x86_64"
#elif defined(_M_IX86) || defined(__i386__)
#define ARCH_NAME "x86"
#elif defined(_M_ARM64) || defined(__aarch64__)
#define ARCH_NAME "aarch64"
#else
#define ARCH_NAME "unknown"
#endif

static void get_hostname(char *out, size_t n) {
#ifdef _WIN32
    DWORD len = (DWORD)n;
    if (!GetComputerNameA(out, &len)) {
        strncpy(out, "unknown", n);
        out[n - 1] = '\0';
    }
#else
    if (gethostname(out, n) != 0) {
        strncpy(out, "unknown", n);
        out[n - 1] = '\0';
    }
#endif
}

/* UDP connect 目标地址，反查本机出口 IP（不发送任何包）。返回 0 成功。 */
static int outbound_ip(const char *target, char *out, size_t n) {
    char host[256], port[16];
    const char *colon = strrchr(target, ':');
    if (!colon || (size_t)(colon - target) >= sizeof(host)) return -1;
    memcpy(host, target, (size_t)(colon - target));
    host[colon - target] = '\0';
    strncpy(port, colon + 1, sizeof(port) - 1);
    port[sizeof(port) - 1] = '\0';

#ifdef _WIN32
    /* 调用方保证 WSA 已初始化（见 main） */
#endif
    struct addrinfo hints, *res = NULL;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_DGRAM;
    int rc = -1;
    if (getaddrinfo(host, port, &hints, &res) == 0 && res) {
#ifdef _WIN32
        SOCKET s = socket(res->ai_family, res->ai_socktype, res->ai_protocol);
        if (s != INVALID_SOCKET) {
            if (connect(s, res->ai_addr, (int)res->ai_addrlen) == 0) {
                struct sockaddr_storage local;
                int len = sizeof(local);
                if (getsockname(s, (struct sockaddr *)&local, &len) == 0) {
                    if (getnameinfo((struct sockaddr *)&local, len, out, (DWORD)n,
                                    NULL, 0, NI_NUMERICHOST) == 0)
                        rc = 0;
                }
            }
            closesocket(s);
        }
    }
#else
        int s = socket(res->ai_family, res->ai_socktype, res->ai_protocol);
        if (s >= 0) {
            if (connect(s, res->ai_addr, res->ai_addrlen) == 0) {
                struct sockaddr_storage local;
                socklen_t len = sizeof(local);
                if (getsockname(s, (struct sockaddr *)&local, &len) == 0) {
                    if (getnameinfo((struct sockaddr *)&local, len, out, (socklen_t)n,
                                    NULL, 0, NI_NUMERICHOST) == 0)
                        rc = 0;
                }
            }
            close(s);
        }
    }
#endif
    if (res) freeaddrinfo(res);
    return rc;
}

static int is_loopback_str(const char *ip) {
    return strncmp(ip, "127.", 4) == 0 || strcmp(ip, "::1") == 0;
}

/* --detail：枚举全部网卡单播地址 */
static void list_interfaces(int as_json) {
#ifdef _WIN32
    ULONG size = 16 * 1024;
    IP_ADAPTER_ADDRESSES *addrs = (IP_ADAPTER_ADDRESSES *)malloc(size);
    if (!addrs) return;
    if (GetAdaptersAddresses(AF_UNSPEC, GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER,
                             NULL, addrs, &size) == ERROR_BUFFER_OVERFLOW) {
        free(addrs);
        addrs = (IP_ADAPTER_ADDRESSES *)malloc(size);
        if (!addrs) return;
        if (GetAdaptersAddresses(AF_UNSPEC, GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER,
                                 NULL, addrs, &size) != NO_ERROR) {
            free(addrs);
            return;
        }
    }
    if (as_json) printf("  \"interfaces\": [\n");
    int first = 1;
    for (IP_ADAPTER_ADDRESSES *a = addrs; a; a = a->Next) {
        if (a->OperStatus != IfOperStatusUp) continue;
        char name[256];
        WideCharToMultiByte(CP_UTF8, 0, a->FriendlyName, -1, name, sizeof(name), NULL, NULL);
        for (IP_ADAPTER_UNICAST_ADDRESS *u = a->FirstUnicastAddress; u; u = u->Next) {
            char ip[INET6_ADDRSTRLEN] = {0};
            DWORD iplen = sizeof(ip);
            if (WSAAddressToStringA(u->Address.lpSockaddr, u->Address.iSockaddrLength,
                                    NULL, ip, &iplen) != 0)
                continue;
            if (as_json) {
                printf("    %s{\"iface\": \"%s\", \"ip\": \"%s\"}\n", first ? "" : ",", name, ip);
                first = 0;
            } else {
                printf("  %-24s %s\n", name, ip);
            }
        }
    }
    if (as_json) printf("  ]\n");
    free(addrs);
#else
    struct ifaddrs *ifap = NULL;
    if (getifaddrs(&ifap) != 0) return;
    if (as_json) printf("  \"interfaces\": [\n");
    int first = 1;
    for (struct ifaddrs *p = ifap; p; p = p->ifa_next) {
        if (!p->ifa_addr) continue;
        int fam = p->ifa_addr->sa_family;
        if (fam != AF_INET && fam != AF_INET6) continue;
        char ip[INET6_ADDRSTRLEN] = {0};
        if (getnameinfo(p->ifa_addr,
                        fam == AF_INET ? sizeof(struct sockaddr_in)
                                       : sizeof(struct sockaddr_in6),
                        ip, sizeof(ip), NULL, 0, NI_NUMERICHOST) != 0)
            continue;
        if (as_json) {
            printf("    %s{\"iface\": \"%s\", \"ip\": \"%s\"}\n", first ? "" : ",", p->ifa_name, ip);
            first = 0;
        } else {
            printf("  %-16s %s\n", p->ifa_name, ip);
        }
    }
    if (as_json) printf("  ]\n");
    freeifaddrs(ifap);
#endif
}

static void usage(const char *prog) {
    printf("跨平台 IP 查看\n\nUsage: %s [OPTIONS]\n\nOptions:\n"
           "      --target <ip:port>  探测出口 IP 的目标（只 connect 不发包） [default: 8.8.8.8:80]\n"
           "      --detail            列出全部网卡地址\n"
           "      --json              单行 JSON 输出\n"
           "  -h, --help              打印帮助\n",
           prog);
}

int main(int argc, char **argv) {
#ifdef _WIN32
    WSADATA wsa;
    SetConsoleOutputCP(CP_UTF8);
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
        fprintf(stderr, "WSAStartup 失败\n");
        return 1;
    }
#endif
    const char *target = "8.8.8.8:80";
    int detail = 0, as_json = 0;
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--detail") == 0) detail = 1;
        else if (strcmp(argv[i], "--json") == 0) as_json = 1;
        else if (strcmp(argv[i], "--target") == 0 && i + 1 < argc) target = argv[++i];
        else if (strcmp(argv[i], "-h") == 0 || strcmp(argv[i], "--help") == 0) {
            usage(argv[0]);
            return 0;
        } else {
            fprintf(stderr, "未知参数: %s\n", argv[i]);
            usage(argv[0]);
            return 1;
        }
    }

    char host[256], ip[INET6_ADDRSTRLEN] = "unreachable";
    get_hostname(host, sizeof(host));
    outbound_ip(target, ip, sizeof(ip));

    if (as_json) {
        printf("{\"hostname\": \"%s\", \"os\": \"%s\", \"arch\": \"%s\", "
               "\"local_ip\": \"%s\", \"is_loopback\": %s, \"target\": \"%s\"%s\n",
               host, OS_NAME, ARCH_NAME, ip,
               is_loopback_str(ip) ? "true" : "false", target,
               detail ? "," : "}");
        if (detail) list_interfaces(1);
        if (detail) printf("}\n");
    } else {
        printf("%s/%s %s -> %s\n", OS_NAME, ARCH_NAME, host, ip);
        if (detail) {
            printf("interfaces:\n");
            list_interfaces(0);
        }
    }
#ifdef _WIN32
    WSACleanup();
#endif
    return 0;
}







/* touch 1 */

/* touch 2 */

/* touch 3 */
