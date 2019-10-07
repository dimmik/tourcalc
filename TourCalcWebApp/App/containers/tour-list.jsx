import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'
import Cookies from 'js-cookie';
import { BrowserRouter as Router, Route, Switch, Link, Redirect } from 'react-router-dom';
import TourAdd from './tour-add.jsx'
import TourNameEdit from './tour-rename.jsx'

import Table from '@material-ui/core/Table';
import TableBody from '@material-ui/core/TableBody';
import TableCell from '@material-ui/core/TableCell';
import TableHead from '@material-ui/core/TableHead';
import TableRow from '@material-ui/core/TableRow';
import Button from '@material-ui/core/Button';


export default class TourList extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isToursLoaded: false,
            tours: null
        }
    }
    componentDidMount() {
        document.title = "Tourcalc: List of tours"
        AppState.loadTours(this);
    }

    render() {
        if (!this.state.isToursLoaded) {
            return <div>Loading Tours ...</div>
        } else {
            return (
                <div>
                    <Table border={0} stickyHeader>
                        <TableHead>
                            <TableRow>
                                <TableCell>Tours
                                </TableCell>
                                <TableCell>Mode: {this.props.authData.type}</TableCell>
                                <TableCell>{
                                    this.props.authData.type === 'Master'
                                        ? <TourAdd buttonText="Add" actionButtonText="Add Tour" app={this} open={false} chooseCode={true}>
                                            <Button color='primary' variant='outlined'>Add Tour</Button>
                                        </TourAdd>
                                        : (
                                            this.state.tours.length > 0
                                                ? <TourAdd buttonText="Add" actionButtonText="Add Tour" app={this} open={false} chooseCode={false}>
                                                    <Button color='primary' variant='outlined'>Add Tour</Button>
                                                </TourAdd>
                                                : <span />
                                        )
                                }</TableCell>
                            </TableRow>
                        </TableHead>
                        <TableBody>
                            {
                                this.state.tours.map((t, idx) => {
                                    return (
                                        <TableRow key={'row' + idx} hover>
                                            <TableCell>
                                            {
                                                this.props.authData.type === 'Master' ? (
                                                    <span key={'s' + idx} style={{ cursor: "pointer", borderStyle: 'ridge', fontSize: "xx-small" }} onClick={() => {
                                                        if (window.confirm('Sure to delete tour ' + t.name + ' (id: ' + t.id + ')?')) {
                                                            AppState.deleteTour(this, t.id)
                                                                .then(() => { AppState.loadTours(this); })
                                                        }
                                                    }}>X</span>) : <span />
                                                }
                                                <u key={'u' + idx}><TourNameEdit key={'te' + idx} tourid={t.id} name={t.name} app={this} open={false} buttonText='Edit' actionButtonText="Change name" /></u>
                                            </TableCell>
                                            <TableCell>
                                                {idx + 1}.<Link key={'l' + idx} to={'/tour/' + t.id}>{t.name}</Link>
                                            </TableCell>
                                            <TableCell>
                                                <Button variant='outlined' onClick={() => { document.getElementById('TourJsonTextArea').value = JSON.stringify(t, null, 2); }}>JSON</Button>
                                            </TableCell>
                                  </TableRow>
                                    )
                                })
                            }

                        </TableBody>
                    </Table>

                    <hr />
                    Tour JSON:
                    <textarea id="TourJsonTextArea" style={{ width: "100%" }} rows="7" defaultValue="Here will be tour JSON"/>
                </div>
                )
        }
    }

};