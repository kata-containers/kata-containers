#!/usr/bin/env Rscript
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Display details for `fio` random read storage IO tests.


library(ggplot2)	# ability to plot nicely
library(gridExtra)	# So we can plot multiple graphs together
suppressMessages(suppressWarnings(library(ggpubr)))	# for ggtexttable
suppressMessages(library(jsonlite))			# to load the data
suppressMessages(suppressWarnings(library(tidyr)))	# for gather
library(tibble)

resultsfiles=c(
	"fio-randread-128.json",
	"fio-randread-256.json",
	"fio-randread-512.json",
	"fio-randread-1k.json",
	"fio-randread-2k.json",
	"fio-randread-4k.json",
	"fio-randread-8k.json",
	"fio-randread-16k.json",
	"fio-randread-32k.json",
	"fio-randread-64k.json"
	)

data2=c()
all_ldata=c()
all_ldata2=c()
stats=c()
rstats=c()
rstats_names=c()

# For each set of results
for (currentdir in resultdirs) {
	dirstats=c()
	for (resultsfile in resultsfiles) {
		fname=paste(inputdir, currentdir, resultsfile, sep="")
		if ( !file.exists(fname)) {
			#warning(paste("Skipping non-existent file: ", fname))
			next
		}

		# Derive the name from the test result dirname
		datasetname=basename(currentdir)

		# Import the data
		fdata=fromJSON(fname)

		blocksize=fdata$Raw$'global options'$bs

		# Extract the latency data - it comes as a table of percentiles, so
		# we have to do a little work...
		clat=data.frame(clat_ns=fdata$Raw$jobs[[1]]$read$clat_ns$percentile)

		# Generate a clat data set with 'clean' percentile numbers so
		# we can sensibly plot it later on.
		clat2=clat
		colnames(clat2)<-sub("clat_ns.", "", colnames(clat2))
		colnames(clat2)<-sub("0000", "", colnames(clat2))
		ldata2=gather(clat2)
		colnames(ldata2)[colnames(ldata2)=="key"] <- "percentile"
		colnames(ldata2)[colnames(ldata2)=="value"] <- "ms"
		ldata2$ms=ldata2$ms/1000000	#ns->ms
		ldata2=cbind(ldata2, runtime=rep(datasetname, length(ldata2$percentile)))
		ldata2=cbind(ldata2, blocksize=rep(blocksize, length(ldata2$percentile)))

		# Pull the 95 and 99 percentile numbers for the boxplot
		# Plotting all values for all runtimes and blocksizes is just way too
		# noisy to make a meaninful picture, so we use this subset.
		# Our values fall more in the range of ms...
		pc95data=tibble(percentile=clat$clat_ns.95.000000/1000000)
		pc95data=cbind(pc95data, runtime=rep(paste(datasetname, "95pc", sep="-"), length(pc95data$percentile)))
		pc99data=tibble(percentile=clat$clat_ns.99.000000/1000000)
		pc99data=cbind(pc99data, runtime=rep(paste(datasetname, "99pc", sep="-"), length(pc95data$percentile)))
		ldata=rbind(pc95data, pc99data)
		ldata=cbind(ldata, blocksize=rep(blocksize, length(ldata$percentile)))

		# We want total bandwidth, so that is the sum of the bandwidths
		# from all the read 'jobs'.
		mdata=data.frame(read_bw_mps=as.numeric(sum(fdata$Raw$jobs[[1]]$read$bw)/1024))
		mdata=cbind(mdata, iops_tot=as.numeric(sum(fdata$Raw$jobs[[1]]$read$iops)))
		mdata=cbind(mdata, runtime=rep(datasetname, length(mdata[, "read_bw_mps"]) ))
		mdata=cbind(mdata, blocksize=rep(blocksize, length(mdata[, "read_bw_mps"]) ))

		# Collect up as sets across all files and runtimes.
		data2=rbind(data2, mdata)
		all_ldata=rbind(all_ldata, ldata)
		all_ldata2=rbind(all_ldata2, ldata2)
	}
}

# Bandwidth line plot
read_bw_line_plot <- ggplot() +
	geom_line( data=data2, aes(blocksize, read_bw_mps, group=runtime, color=runtime)) +
	ylim(0, NA) +
	ggtitle("Random Read total bandwidth") +
	xlab("Blocksize") +
	ylab("Bandwidth (MiB/s)") +
	theme(
		axis.text.x=element_text(angle=90),
		legend.position=c(0.35,0.8),
		legend.title=element_text(size=5),
		legend.text=element_text(size=5),
		legend.background = element_rect(fill=alpha('blue', 0.2))
	)

# IOPS line plot
read_iops_line_plot <- ggplot() +
	geom_line( data=data2, aes(blocksize, iops_tot, group=runtime, color=runtime)) +
	ylim(0, NA) +
	ggtitle("Random Read total IOPS") +
	xlab("Blocksize") +
	ylab("IOPS") +
	theme(
		axis.text.x=element_text(angle=90),
		legend.position=c(0.35,0.8),
		legend.title=element_text(size=5),
		legend.text=element_text(size=5),
		legend.background = element_rect(fill=alpha('blue', 0.2))
	)

# 95 and 99 percentile box plot
read_clat_box_plot <- ggplot() +
	geom_boxplot( data=all_ldata, aes(blocksize, percentile, color=runtime)) +
	ylim(0, NA) +
	ggtitle("Random Read completion latency", subtitle="95&98 percentiles, boxplot over jobs") +
	xlab("Blocksize") +
	ylab("Latency (ms)") +
	theme(axis.text.x=element_text(angle=90)) +
	# Use the 'paired' colour matrix as we are setting these up as pairs of
	# 95 and 99 percentiles, and it is much easier to visually group those to
	# each runtime if we use this colourmap.
	scale_colour_brewer(palette="Paired")
#	it would be nice to use the same legend theme as the other plots on this
#	page, but because of the number of entries it tends to flow off the picture.
#	theme(
#		axis.text.x=element_text(angle=90),
#		legend.position=c(0.35,0.8),
#		legend.title=element_text(size=5),
#		legend.text=element_text(size=5),
#		legend.background = element_rect(fill=alpha('blue', 0.2))
#	)

# As the boxplot is actually quite hard to interpret, also show a linegraph
# of all the percentiles for a single blocksize.
which_blocksize='4k'
clat_line_subtitle=paste("For blocksize", which_blocksize, sep=" ")
single_blocksize=subset(all_ldata2, blocksize==which_blocksize)
clat_line=aggregate(
	single_blocksize$ms,
	by=list(
		percentile=single_blocksize$percentile,
		blocksize=single_blocksize$blocksize,
		runtime=single_blocksize$runtime
	),
	FUN=mean
)

clat_line$percentile=as.numeric(clat_line$percentile)

read_clat_line_plot <- ggplot() +
	geom_line( data=clat_line, aes(percentile, x, group=runtime, color=runtime)) +
	ylim(0, NA) +
	ggtitle("Random Read completion latency percentiles", subtitle=clat_line_subtitle) +
	xlab("Percentile") +
	ylab("Time (ms)") +
	theme(
		axis.text.x=element_text(angle=90),
		legend.position=c(0.35,0.8),
		legend.title=element_text(size=5),
		legend.text=element_text(size=5),
		legend.background = element_rect(fill=alpha('blue', 0.2))
	)

master_plot = grid.arrange(
	read_bw_line_plot,
	read_iops_line_plot,
	read_clat_box_plot,
	read_clat_line_plot,
	nrow=2,
	ncol=2 )

