#!/usr/bin/env Rscript
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Show system memory reduction, and hence container 'density', by analysing the
# scaling footprint data results and the 'system free' memory.

library(ggplot2)					# ability to plot nicely.
							# So we can plot multiple graphs
library(gridExtra)					# together.
suppressMessages(suppressWarnings(library(ggpubr)))	# for ggtexttable.
suppressMessages(library(jsonlite))			# to load the data.

testnames=c(
	paste("footprint-busybox.*", test_name_extra, sep=""),
	paste("footprint-mysql.*", test_name_extra, sep=""),
	paste("footprint-elasticsearch.*", test_name_extra, sep="")
)

data=c()
stats=c()
rstats=c()
rstats_names=c()

for (currentdir in resultdirs) {
	count=1
	dirstats=c()
	for (testname in testnames) {
		matchdir=paste(inputdir, currentdir, sep="")
		matchfile=paste(testname, '\\.json', sep="")
		files=list.files(matchdir, pattern=matchfile)
		if ( length(files) == 0 ) {
			#warning(paste("Pattern [", matchdir, "/", matchfile, "] matched nothing"))
		}
		for (ffound in files) {
			fname=paste(inputdir, currentdir, ffound, sep="")
			if ( !file.exists(fname)) {
				warning(paste("Skipping non-existent file: ", fname))
				next
			}

			# Derive the name from the test result dirname
			datasetname=basename(currentdir)

			# Import the data
			fdata=fromJSON(fname)
			# De-nest the test name specific data
			shortname=substr(ffound, 1, nchar(ffound)-nchar(".json"))
			fdata=fdata[[shortname]]

			payload=fdata$Config$payload
			testname=paste(datasetname, payload)

			cdata=data.frame(avail_mb=as.numeric(fdata$Results$system$avail)/(1024*1024))
			cdata=cbind(cdata, avail_decr=as.numeric(fdata$Results$system$avail_decr))
			cdata=cbind(cdata, count=seq_len(length(cdata[, "avail_mb"])))
			cdata=cbind(cdata, testname=rep(testname, length(cdata[, "avail_mb"]) ))
			cdata=cbind(cdata, payload=rep(payload, length(cdata[, "avail_mb"]) ))
			cdata=cbind(cdata, dataset=rep(datasetname, length(cdata[, "avail_mb"]) ))

			# Gather our statistics
			sdata=data.frame(num_containers=length(cdata[, "avail_mb"]))
			# Pick out the last avail_decr value - which in theory should be
			# the most we have consumed...
			sdata=cbind(sdata, mem_consumed=cdata[, "avail_decr"][length(cdata[, "avail_decr"])])
			sdata=cbind(sdata, avg_bytes_per_c=sdata$mem_consumed / sdata$num_containers)
			sdata=cbind(sdata, runtime=testname)

			# Store away as a single set
			data=rbind(data, cdata)
			stats=rbind(stats, sdata)

			s = c(
				"Test"=testname,
				"n"=sdata$num_containers,
				"size"=(sdata$mem_consumed) / 1024,
				"kb/n"=round((sdata$mem_consumed / sdata$num_containers) / 1024, digits=1),
				"n/Gb"= round((1*1024*1024*1024) / (sdata$mem_consumed / sdata$num_containers), digits=1)
			)

			rstats=rbind(rstats, s)
			count = count + 1
		}
	}
}

# Set up the text table headers
colnames(rstats)=c("Test", "n", "Tot_Kb", "avg_Kb", "n_per_Gb")

# Build us a text table of numerical results
stats_plot = suppressWarnings(ggtexttable(data.frame(rstats),
	theme=ttheme(base_size=10),
	rows=NULL
	))

# plot how samples varioed over  'time'
line_plot <- ggplot() +
	geom_point( data=data, aes(count, avail_mb, group=testname, color=payload, shape=dataset)) +
	geom_line( data=data, aes(count, avail_mb, group=testname, color=payload)) +
	xlab("Containers") +
	ylab("System Avail (Mb)") +
	ggtitle("System Memory free") +
	ylim(0, NA) +
	theme(axis.text.x=element_text(angle=90))

master_plot = grid.arrange(
	line_plot,
	stats_plot,
	nrow=2,
	ncol=1 )

