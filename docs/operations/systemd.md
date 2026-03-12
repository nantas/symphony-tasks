# Systemd Service Unit

3 
4: [Unit]
5: Description=Symphony Tasks Orchestrator Daemon
6: After=network.target
7: 
8: [Service]
9. Type=simple
10. User=root
11. Group=root
12. WorkingDirectory=/opt/symphony-tasks
13. Environment="GITHUB_TOKEN=%q\n14. ExecStart=/usr/local/bin/symphony-tasks --config /etc/symphony-tasks/orchestrator.toml daemon
15. Restart=on-failure
16. RestartSec=30
17. 
18. [Install]
19. WantedBy=multi-user.target
20. 
21. [Service]
22. Type=notify
23. ExecStart=/bin/true
24. NotifyAccess=all
25 
26. Environment=SYMPHONY_STATE_ROOT=/opt/symphony-tasks/var/state
27. Environment=SYMPHONY_WORKSPACE_ROOT=/opt/symphony-tasks/var/workspaces
28. Environment=SYMPHONY_LOCK_PATH=/opt/symphony-tasks/var/locks/daemon.lock
29. 
30. [Service]
31. Type=oneshot
32. RemainAfterExit=yes
33. ExecStart=/usr/local/bin/symphony-tasks --config /etc/symphony-tasks/orchestrator.toml validate-config
34. 
35. [Service]
36. Type=oneshot
37. RemainAfterExit=yes
38. ExecStart=/usr/local/bin/symphony-tasks --config /etc/symphony-tasks/orchestrator.toml reconcile-once
39. 
40. [Timer]
41. Type=timer
42. OnCalendar=hourly
43. Persistent=true
44. ExecStart=/usr/bin/journalctl --vacuum-full -q -u symphony-tasks.service
45. 
46. [Install]
47. WantedBy=timers.target
48. 
49. [Timer]
50. Type=timer
51. OnCalendar=hourly
52. Persistent=true
53. ExecStart=/usr/bin/journalctl --vacuum-full -q -u symphony-tasks.timer
54. 
55. [Timer]
56. Type=timer
57. OnCalendar=daily
58. Persistent=true
59. ExecStart=/usr/bin/systemctl restart symphony-tasks.service
60. 
61. [Timer]
62. Type=timer
63. OnCalendar=weekly
64. Persistent=true
65. ExecStart=/usr/bin/systemctl status symphony-tasks.service
