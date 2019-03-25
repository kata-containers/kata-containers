// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"fmt"
	"net"
	"os"
	"os/signal"
	"path/filepath"
	"syscall"

	"github.com/gogo/protobuf/types"
	pb "github.com/kata-containers/runtime/protocols/cache"
	vc "github.com/kata-containers/runtime/virtcontainers"
	vf "github.com/kata-containers/runtime/virtcontainers/factory"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/pkg/errors"
	"github.com/urfave/cli"
	"golang.org/x/sys/unix"
	"google.golang.org/grpc"
)

var factorySubCmds = []cli.Command{
	initFactoryCommand,
	destroyFactoryCommand,
	statusFactoryCommand,
}

var factoryCLICommand = cli.Command{
	Name:        "factory",
	Usage:       "manage vm factory",
	Subcommands: factorySubCmds,
	Action: func(context *cli.Context) {
		cli.ShowSubcommandHelp(context)
	},
}

type cacheServer struct {
	rpc     *grpc.Server
	factory vc.Factory
}

var jsonVMConfig *pb.GrpcVMConfig

// Config requests base factory config and convert it to gRPC protocol.
func (s *cacheServer) Config(ctx context.Context, empty *types.Empty) (*pb.GrpcVMConfig, error) {
	if jsonVMConfig == nil {
		config := s.factory.Config()

		var err error
		jsonVMConfig, err = config.ToGrpc()
		if err != nil {
			return nil, err
		}
	}

	return jsonVMConfig, nil
}

// GetBaseVM requests a paused VM and convert it to gRPC protocol.
func (s *cacheServer) GetBaseVM(ctx context.Context, empty *types.Empty) (*pb.GrpcVM, error) {
	config := s.factory.Config()

	vm, err := s.factory.GetBaseVM(ctx, config)
	if err != nil {
		return nil, errors.Wrapf(err, "failed to GetBaseVM")
	}

	return vm.ToGrpc(config)
}

func getUnixListener(path string) (net.Listener, error) {
	err := os.MkdirAll(filepath.Dir(path), 0755)
	if err != nil {
		return nil, err
	}
	if err = unix.Unlink(path); err != nil && !os.IsNotExist(err) {
		return nil, err
	}
	l, err := net.Listen("unix", path)
	if err != nil {
		return nil, err
	}
	if err = os.Chmod(path, 0600); err != nil {
		l.Close()
		return nil, err
	}
	return l, nil
}

var handledSignals = []os.Signal{
	syscall.SIGTERM,
	syscall.SIGINT,
	syscall.SIGPIPE,
}

func handleSignals(s *cacheServer, signals chan os.Signal) chan struct{} {
	done := make(chan struct{}, 1)
	go func() {
		for {
			sig := <-signals
			kataLog.WithField("signal", sig).Debug("received signal")
			switch sig {
			case unix.SIGPIPE:
				continue
			default:
				s.rpc.GracefulStop()
				close(done)
				return
			}
		}
	}()
	return done
}

var initFactoryCommand = cli.Command{
	Name:  "init",
	Usage: "initialize a VM factory based on kata-runtime configuration",
	Action: func(c *cli.Context) error {
		ctx, err := cliContextToContext(c)
		if err != nil {
			return err
		}

		runtimeConfig, ok := c.App.Metadata["runtimeConfig"].(oci.RuntimeConfig)
		if !ok {
			return errors.New("invalid runtime config")
		}

		if runtimeConfig.FactoryConfig.VMCacheNumber > 0 {
			factoryConfig := vf.Config{
				Template: runtimeConfig.FactoryConfig.Template,
				Cache:    runtimeConfig.FactoryConfig.VMCacheNumber,
				VMCache:  true,
				VMConfig: vc.VMConfig{
					HypervisorType:   runtimeConfig.HypervisorType,
					HypervisorConfig: runtimeConfig.HypervisorConfig,
					AgentType:        runtimeConfig.AgentType,
					AgentConfig:      runtimeConfig.AgentConfig,
					ProxyType:        runtimeConfig.ProxyType,
					ProxyConfig:      runtimeConfig.ProxyConfig,
				},
			}
			f, err := vf.NewFactory(ctx, factoryConfig, false)
			if err != nil {
				return err
			}
			defer f.CloseFactory(ctx)

			s := &cacheServer{
				rpc:     grpc.NewServer(),
				factory: f,
			}
			pb.RegisterCacheServiceServer(s.rpc, s)

			l, err := getUnixListener(runtimeConfig.FactoryConfig.VMCacheEndpoint)
			if err != nil {
				return err
			}
			defer l.Close()

			signals := make(chan os.Signal, 8)
			done := handleSignals(s, signals)
			signal.Notify(signals, handledSignals...)

			kataLog.WithField("endpoint", runtimeConfig.FactoryConfig.VMCacheEndpoint).Info("VM cache server start")
			s.rpc.Serve(l)

			<-done

			kataLog.WithField("endpoint", runtimeConfig.FactoryConfig.VMCacheEndpoint).Info("VM cache server stop")
			return nil
		}

		if runtimeConfig.FactoryConfig.Template {
			factoryConfig := vf.Config{
				Template: true,
				VMConfig: vc.VMConfig{
					HypervisorType:   runtimeConfig.HypervisorType,
					HypervisorConfig: runtimeConfig.HypervisorConfig,
					AgentType:        runtimeConfig.AgentType,
					AgentConfig:      runtimeConfig.AgentConfig,
					ProxyType:        runtimeConfig.ProxyType,
				},
			}
			kataLog.WithField("factory", factoryConfig).Info("create vm factory")
			_, err := vf.NewFactory(ctx, factoryConfig, false)
			if err != nil {
				kataLog.WithError(err).Error("create vm factory failed")
				return err
			}
			fmt.Fprintln(defaultOutputFile, "vm factory initialized")
		} else {
			kataLog.Error("vm factory is not enabled")
			fmt.Fprintln(defaultOutputFile, "vm factory is not enabled")
		}

		return nil
	},
}

var destroyFactoryCommand = cli.Command{
	Name:  "destroy",
	Usage: "destroy the VM factory",
	Action: func(c *cli.Context) error {
		ctx, err := cliContextToContext(c)
		if err != nil {
			return err
		}

		runtimeConfig, ok := c.App.Metadata["runtimeConfig"].(oci.RuntimeConfig)
		if !ok {
			return errors.New("invalid runtime config")
		}

		if runtimeConfig.FactoryConfig.Template {
			factoryConfig := vf.Config{
				Template: true,
				VMConfig: vc.VMConfig{
					HypervisorType:   runtimeConfig.HypervisorType,
					HypervisorConfig: runtimeConfig.HypervisorConfig,
					AgentType:        runtimeConfig.AgentType,
					AgentConfig:      runtimeConfig.AgentConfig,
				},
			}
			kataLog.WithField("factory", factoryConfig).Info("load vm factory")
			f, err := vf.NewFactory(ctx, factoryConfig, true)
			if err != nil {
				kataLog.WithError(err).Error("load vm factory failed")
				// ignore error
			} else {
				f.CloseFactory(ctx)
			}
		}
		fmt.Fprintln(defaultOutputFile, "vm factory destroyed")
		return nil
	},
}

var statusFactoryCommand = cli.Command{
	Name:  "status",
	Usage: "query the status of VM factory",
	Action: func(c *cli.Context) error {
		ctx, err := cliContextToContext(c)
		if err != nil {
			return err
		}

		runtimeConfig, ok := c.App.Metadata["runtimeConfig"].(oci.RuntimeConfig)
		if !ok {
			return errors.New("invalid runtime config")
		}

		if runtimeConfig.FactoryConfig.Template {
			factoryConfig := vf.Config{
				Template: true,
				VMConfig: vc.VMConfig{
					HypervisorType:   runtimeConfig.HypervisorType,
					HypervisorConfig: runtimeConfig.HypervisorConfig,
					AgentType:        runtimeConfig.AgentType,
					AgentConfig:      runtimeConfig.AgentConfig,
				},
			}
			kataLog.WithField("factory", factoryConfig).Info("load vm factory")
			_, err := vf.NewFactory(ctx, factoryConfig, true)
			if err != nil {
				fmt.Fprintln(defaultOutputFile, "vm factory is off")
			} else {
				fmt.Fprintln(defaultOutputFile, "vm factory is on")
			}
		} else {
			fmt.Fprintln(defaultOutputFile, "vm factory not enabled")
		}
		return nil
	},
}
