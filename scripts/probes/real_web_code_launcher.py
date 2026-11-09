#!/usr/bin/env python3
"""Aplica límites al proceso Deno sin usar preexec_fn en un host con hilos."""
import os
import resource
import sys


def main():
    executable=sys.argv[1]
    timeout=int(sys.argv[2])
    args=sys.argv[3:]
    resource.setrlimit(resource.RLIMIT_FSIZE,(1_000_000,1_000_000))
    resource.setrlimit(resource.RLIMIT_NOFILE,(64,64))
    resource.setrlimit(resource.RLIMIT_CPU,(timeout,timeout+2))
    os.execv(executable,[executable,*args])


if __name__=='__main__':main()
