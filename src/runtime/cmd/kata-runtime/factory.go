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
	"time"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	pb "github.com/kata-containers/kata-containers/src/runtime/protocols/cache"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	vf "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/factory"
	"github.com/pkg/errors"
	"github.com/urfave/cli"
	"golang.org/x/sys/unix"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	emptypb "google.golang.org/protobuf/types/known/emptypb"
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
	done    chan struct{}
	pb.UnimplementedCacheServiceServer
}

var jsonVMConfig *pb.GrpcVMConfig

// Config requests base factory config and convert it to gRPC protocol.
func (s *cacheServer) Config(ctx context.Context, empty *emptypb.Empty) (*pb.GrpcVMConfig, error) {
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
func (s *cacheServer) GetBaseVM(ctx context.Context, empty *emptypb.Empty) (*pb.GrpcVM, error) {
	config := s.factory.Config()

	vm, err := s.factory.GetBaseVM(ctx, config)
	if err != nil {
		return nil, errors.Wrapf(err, "failed to GetBaseVM")
	}

	return vm.ToGrpc(ctx, config)
}

func (s *cacheServer) quit() {
	s.rpc.GracefulStop()
	close(s.done)
}

// Quit will stop VMCache server after 1 second.
func (s *cacheServer) Quit(ctx context.Context, empty *emptypb.Empty) (*emptypb.Empty, error) {
	go func() {
		kataLog.Info("VM cache server will stop after 1 second")
		time.Sleep(time.Second)
		s.quit()
	}()
	return &emptypb.Empty{}, nil
}

func (s *cacheServer) Status(ctx context.Context, empty *emptypb.Empty) (*pb.GrpcStatus, error) {
	stat := pb.GrpcStatus{
		Pid:      int64(os.Getpid()),
		Vmstatus: s.factory.GetVMStatus(),
	}
	return &stat, nil
}

func getUnixListener(path string) (net.Listener, error) {
	err := os.MkdirAll(filepath.Dir(path), 0755)
	if err != nil {
		return nil, err
	}
	_, err = os.Stat(path)
	if err == nil {
		return nil, fmt.Errorf("%s already exist.  Please stop running VMCache server and remove %s", path, path)
	} else if !os.IsNotExist(err) {
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

func handleSignals(s *cacheServer, signals chan os.Signal) {
	s.done = make(chan struct{}, 1)
	go func() {
		for {
			sig := <-signals
			kataLog.WithField("signal", sig).Debug("received signal")
			switch sig {
			case unix.SIGPIPE:
				continue
			default:
				s.quit()
				return
			}
		}
	}()
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

		factoryConfig := vf.Config{
			Template:     runtimeConfig.FactoryConfig.Template,
			TemplatePath: runtimeConfig.FactoryConfig.TemplatePath,
			Cache:        runtimeConfig.FactoryConfig.VMCacheNumber,
			VMCache:      runtimeConfig.FactoryConfig.VMCacheNumber > 0,
			VMConfig: vc.VMConfig{
				HypervisorType:   runtimeConfig.HypervisorType,
				HypervisorConfig: runtimeConfig.HypervisorConfig,
				AgentConfig:      runtimeConfig.AgentConfig,
			},
		}

		if runtimeConfig.FactoryConfig.VMCacheNumber > 0 {
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
			handleSignals(s, signals)
			signal.Notify(signals, handledSignals...)

			kataLog.WithField("endpoint", runtimeConfig.FactoryConfig.VMCacheEndpoint).Info("VM cache server start")
			s.rpc.Serve(l)

			<-s.done

			kataLog.WithField("endpoint", runtimeConfig.FactoryConfig.VMCacheEndpoint).Info("VM cache server stop")
			return nil
		}

		if runtimeConfig.FactoryConfig.Template {
			kataLog.WithField("factory", factoryConfig).Info("create vm factory")
			_, err := vf.NewFactory(ctx, factoryConfig, false)
			if err != nil {
				kataLog.WithError(err).Error("create vm factory failed")
				return err
			}
			fmt.Fprintln(defaultOutputFile, "vm factory initialized")
		} else {
			const errstring = "vm factory or VMCache is not enabled"
			kataLog.Error(errstring)
			fmt.Fprintln(defaultOutputFile, errstring)
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

		if runtimeConfig.FactoryConfig.VMCacheNumber > 0 {
			conn, err := grpc.Dial(fmt.Sprintf("unix://%s", runtimeConfig.FactoryConfig.VMCacheEndpoint), grpc.WithTransportCredentials(insecure.NewCredentials()))
			if err != nil {
				return errors.Wrapf(err, "failed to connect %q", runtimeConfig.FactoryConfig.VMCacheEndpoint)
			}
			defer conn.Close()
			_, err = pb.NewCacheServiceClient(conn).Quit(ctx, &emptypb.Empty{})
			if err != nil {
				return errors.Wrapf(err, "failed to call gRPC Quit")
			}
			// Wait VMCache server stop
			time.Sleep(time.Second)
		} else if runtimeConfig.FactoryConfig.Template {
			factoryConfig := vf.Config{
				Template:     true,
				TemplatePath: runtimeConfig.FactoryConfig.TemplatePath,
				VMConfig: vc.VMConfig{
					HypervisorType:   runtimeConfig.HypervisorType,
					HypervisorConfig: runtimeConfig.HypervisorConfig,
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

		if runtimeConfig.FactoryConfig.VMCacheNumber > 0 {
			conn, err := grpc.Dial(fmt.Sprintf("unix://%s", runtimeConfig.FactoryConfig.VMCacheEndpoint), grpc.WithTransportCredentials(insecure.NewCredentials()))
			if err != nil {
				fmt.Fprintln(defaultOutputFile, errors.Wrapf(err, "failed to connect %q", runtimeConfig.FactoryConfig.VMCacheEndpoint))
			} else {
				defer conn.Close()
				status, err := pb.NewCacheServiceClient(conn).Status(ctx, &emptypb.Empty{})
				if err != nil {
					fmt.Fprintln(defaultOutputFile, errors.Wrapf(err, "failed to call gRPC Status\n"))
				} else {
					fmt.Fprintf(defaultOutputFile, "VM cache server pid = %d\n", status.Pid)
					for _, vs := range status.Vmstatus {
						fmt.Fprintf(defaultOutputFile, "VM pid = %d Cpu = %d Memory = %dMiB\n", vs.Pid, vs.Cpu, vs.Memory)
					}
				}
			}
		}
		if runtimeConfig.FactoryConfig.Template {
			factoryConfig := vf.Config{
				Template:     true,
				TemplatePath: runtimeConfig.FactoryConfig.TemplatePath,
				VMConfig: vc.VMConfig{
					HypervisorType:   runtimeConfig.HypervisorType,
					HypervisorConfig: runtimeConfig.HypervisorConfig,
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
