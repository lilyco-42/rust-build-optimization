#include <stdio.h>
#include <stddef.h>
#include <winsock2.h>
#include <iphlpapi.h>
int main(void) {
    printf("IP_ADAPTER_ADDRESSES size=%zu\n", sizeof(IP_ADAPTER_ADDRESSES));
    printf("Next=%zu AdapterName=%zu FirstUnicast=%zu FriendlyName=%zu\n",
        offsetof(IP_ADAPTER_ADDRESSES, Next),
        offsetof(IP_ADAPTER_ADDRESSES, AdapterName),
        offsetof(IP_ADAPTER_ADDRESSES, FirstUnicastAddress),
        offsetof(IP_ADAPTER_ADDRESSES, FriendlyName));
    printf("PhysicalAddress=%zu PhysLen=%zu Flags=%zu Mtu=%zu IfType=%zu OperStatus=%zu Ipv6IfIndex=%zu\n",
        offsetof(IP_ADAPTER_ADDRESSES, PhysicalAddress),
        offsetof(IP_ADAPTER_ADDRESSES, PhysicalAddressLength),
        offsetof(IP_ADAPTER_ADDRESSES, Flags),
        offsetof(IP_ADAPTER_ADDRESSES, Mtu),
        offsetof(IP_ADAPTER_ADDRESSES, IfType),
        offsetof(IP_ADAPTER_ADDRESSES, OperStatus),
        offsetof(IP_ADAPTER_ADDRESSES, Ipv6IfIndex));
    printf("UNICAST: size=%zu Next=%zu Address=%zu\n",
        sizeof(IP_ADAPTER_UNICAST_ADDRESS),
        offsetof(IP_ADAPTER_UNICAST_ADDRESS, Next),
        offsetof(IP_ADAPTER_UNICAST_ADDRESS, Address));
    printf("SOCKET_ADDRESS: lpSockaddr=%zu iSockaddrLength=%zu size=%zu\n",
        offsetof(SOCKET_ADDRESS, lpSockaddr),
        offsetof(SOCKET_ADDRESS, iSockaddrLength),
        sizeof(SOCKET_ADDRESS));
    return 0;
}
