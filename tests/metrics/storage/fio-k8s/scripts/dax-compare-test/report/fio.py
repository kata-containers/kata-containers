
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
import pandas as pd
import os
import re
import io
import glob
from IPython.display import display, Markdown
import matplotlib.pyplot as plt

#Compare the tests results group by fio job.
#Input:
# df: dataset from `import_data()`
# metric: string of metrics provided in `df`
def compare_tests_group_by_fio_job(df, metric):
    test_names, metric_df = group_metrics_group_by_testname(df, metric)
    show_df(metric_df)
    plot_df(metric_df,test_names)

# Given a metric return results per test group by fio job.
# input:
#    df: dataset from `import_data()`
#    metric: string with the name of the metric to filter.
# output:
#    dataset with fomat:
#      'workload' , 'name[0]' , ... , 'name[n]'
#
def group_metrics_group_by_testname(df, metric):
  #name of each tests from results
  names = set()
  # Rows of new data set
  rows = []
  # map:
  # keys: name of fio job
  # value: dict[k]:v where k: name of a test, v: value of test for  metric`
  workload = {}

  for k, row in df.iterrows():
    # name of a fio job
    w = row['WORKLOAD']
    # name of tests
    tname = row['NAME']
    names.add(tname)
    # given a fio job name get dict of values
    # if not previous values init empty dict
    dict_values = workload.get(w, {})
    # For a given metric, add it into as value of dict_values[testname]=val
    #e.g
    # dict_values["test-name"] = row["IOPS"]
    dict_values[tname] = row[metric]
    workload[w] = dict_values

  names = list(names)
  cols = ['WORKLOAD'] + list(names)
  rdf = pd.DataFrame(workload,columns = cols)

  for k in workload:
    d = workload[k]

    if not d[names[0]] == 0:
      d["WORKLOAD"] = k;
      rdf = rdf.append(d,ignore_index=True)
  rdf = rdf.dropna()
  return names, rdf

def plot_df(df, names,sort_key=""):
  if sort_key != "":
    df.sort_values(sort_key, ascending=False)
  df.plot(kind='bar',x="WORKLOAD",y=names,  figsize=(30, 10))
  plt.show()


def import_data():
    frames = []
    for f in glob.glob('./results/*/results.csv'):
        print("reading:" + f)
        df = pd.read_csv(f)
        frames.append(df)
    return pd.concat(frames)

def show_df(df):
    pd.set_option('display.max_rows', df.shape[0]+1)
    print(df)

def print_md(s):
     display(Markdown(s))

#notebook entrypoint
def generate_report():
    #Load the all test results in a single dataset
    df_results = import_data()
    print_md("Show all data from results")
    show_df(df_results)
    print_md("### Compare the tests results group by fio job. The metric used to compare is write bandwidth")
    compare_tests_group_by_fio_job(df_results, 'bw_w')
    print_md("### Compare the tests results group by fio job. The metric used to compare is read bandwidth")
    compare_tests_group_by_fio_job(df_results, 'bw_r')
    print_md("### Compare the tests results group by fio job. The metric used to compare is write IOPS(Input/Output Operations Per Second)")
    compare_tests_group_by_fio_job(df_results, 'IOPS_w')
    print_md("### Compare the tests results group by fio job. The metric used to compare is read IOPS(Input/Output Operations Per Second)")
    compare_tests_group_by_fio_job(df_results, 'IOPS_r')
